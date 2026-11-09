//! Central's package activation (#89; configuration.md § Distribution and activation on nodes):
//! every minute, central activates the national set its activation records give it now,
//! verifying each package it published against the national root (`HEALTH_NATIONAL_ROOT_FILE`)
//! from its blob store (`HEALTH_FILES_DIR`). When the set changes, and once at startup, the
//! active packages' definitions are written into the store, where clients read them and from
//! where they sync down to every node.
//!
//! The same set gives the FHIR server its profiles and terminology ([`Definitions`]): the
//! active packages' StructureDefinitions, compiled, validate writes (tiers 3–4), and their
//! CodeSystems, ValueSets and ConceptMaps answer the terminology operations and bindings. A set
//! whose profiles do not compile keeps the previous ones and raises
//! `package-activation-failed`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use std::collections::BTreeMap;

use health_activation::definitions::{active_files, definitions, write_definitions};
use health_activation::{Activator, Outcome, activate};
use health_configuration::activation::{ActiveSet, Chain};
use health_fhir_server::{ActiveProfiles, ActiveTerminology, Profiles};
use health_fhir_validation::ProfileSet;
use health_files::FsBlobStore;
use health_packages::Files;
use health_persistence::Store as _;
use health_surrealdb::{Connection, SurrealStore};
use health_time::{SystemClock, Timestamp};

/// How often central looks for a new set.
const EVERY: Duration = Duration::from_secs(60);

/// What the FHIR server validates and answers with, from the active set.
#[derive(Debug, Clone, Default)]
pub(crate) struct Definitions {
    /// The active packages' profiles.
    pub(crate) profiles: ActiveProfiles,
    /// Their terminology.
    pub(crate) terminology: ActiveTerminology,
}

impl Definitions {
    /// Serves the profiles and terminology of `packages` from now on, or keeps the previous
    /// ones when they do not load.
    pub(crate) fn load(&self, packages: &[(String, Files)]) -> Result<usize, String> {
        let mut terminology = health_terminology::Terminology::new();
        let mut structures = Vec::new();
        for (id, files) in packages {
            health_packages::terminology::load(&mut terminology, files)
                .map_err(|e| format!("{id}: its terminology: {e}"))?;
            structures.extend(
                definitions(files)
                    .into_iter()
                    .filter(|(kind, ..)| kind.as_str() == "StructureDefinition")
                    .map(|(.., resource)| resource),
            );
        }
        let set = ProfileSet::compile(&structures).map_err(|errors| {
            let shown: Vec<String> = errors.iter().map(|e| format!("{e:?}")).collect();
            format!("the active profiles do not compile: {}", shown.join("; "))
        })?;
        let profiles = structures.len();
        self.profiles.replace(Profiles {
            set: Arc::new(set),
            required: BTreeMap::new(),
        });
        self.terminology.replace(terminology);
        Ok(profiles)
    }
}

/// The active set and its definitions, once.
async fn once(
    store: &SurrealStore,
    blobs: &FsBlobStore,
    activator: &Activator,
    served: &Definitions,
    first: bool,
) -> Result<(), String> {
    let report = activate(
        store,
        Some(blobs),
        activator,
        Timestamp::now(),
        |digest| async move {
            Err(format!(
                "{digest} is not in central's blob store: publish the package again"
            ))
        },
    )
    .await?;
    let switched = report
        .as_ref()
        .is_some_and(|r| r.outcome == Outcome::Activated);
    if !(switched || first) {
        return Ok(());
    }
    let mut tx = store.begin().await.map_err(|e| e.to_string())?;
    let active: ActiveSet = match tx.active_packages().await.map_err(|e| e.to_string())? {
        Some(json) => serde_json::from_str(&json).map_err(|e| e.to_string())?,
        None => ActiveSet::new(),
    };
    drop(tx);
    let written = write_definitions(store, blobs, &active).await?;
    if written > 0 {
        tracing::info!(written, "definitions of the active packages written");
    }
    match served.load(&active_files(store, blobs, &active).await?) {
        Ok(profiles) => {
            tracing::info!(
                profiles,
                "serving the active packages' profiles and terminology"
            );
        }
        Err(error) => tracing::error!(
            alert = "package-activation-failed",
            %error,
            "the previous profiles and terminology stay"
        ),
    }
    Ok(())
}

/// Activates the national set every minute, until central stops.
pub(crate) async fn run(
    connection: Connection,
    root: PathBuf,
    files: PathBuf,
    served: Definitions,
) {
    // Indexed with central's search registry, like every write of the FHIR server.
    let store = SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(crate::auth::search_registry());
    let started = (|| {
        let activator = Activator::from_root_file(&root, Chain::default())?;
        let blobs = FsBlobStore::open(&files)
            .map_err(|e| format!("the blob store {}: {e}", files.display()))?;
        Ok::<_, String>((activator, blobs))
    })();
    let (activator, blobs) = match started {
        Ok(started) => started,
        Err(error) => {
            tracing::warn!(%error, "packages are not activated on central");
            return;
        }
    };
    let mut first = true;
    loop {
        if let Err(error) = once(&store, &blobs, &activator, &served, first).await {
            tracing::warn!(%error, "activating the national packages failed");
        } else {
            first = false;
        }
        tokio::time::sleep(EVERY).await;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn files(resources: &[Value]) -> Files {
        resources
            .iter()
            .map(|r| {
                (
                    format!(
                        "{}-{}.json",
                        r["resourceType"].as_str().unwrap_or_default(),
                        r["id"].as_str().unwrap_or_default()
                    ),
                    r.to_string().into_bytes(),
                )
            })
            .collect()
    }

    #[test]
    fn the_active_terminology_is_served_and_a_set_that_does_not_load_is_not() {
        let served = Definitions::default();
        let codes = json!({
            "resourceType": "CodeSystem", "id": "visit-type",
            "url": "http://health.go.ug/fhir/CodeSystem/visit-type", "version": "1.0.0",
            "status": "active", "content": "complete",
            "concept": [{ "code": "first", "display": "First visit" }]
        });
        assert_eq!(
            served.load(&[("ug.moh.core@1.0.0".into(), files(&[codes]))]),
            Ok(0)
        );
        let lookup = |served: &Definitions| {
            served
                .terminology
                .current()
                .lookup(
                    "http://health.go.ug/fhir/CodeSystem/visit-type",
                    None,
                    "first",
                )
                .is_ok()
        };
        assert!(lookup(&served));

        // A profile whose element cannot be enforced does not compile: nothing is replaced.
        let broken = json!({
            "resourceType": "StructureDefinition", "id": "broken",
            "url": "http://health.go.ug/fhir/StructureDefinition/broken", "version": "1.0.0",
            "status": "active", "kind": "resource", "abstract": false, "type": "Patient",
            "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
            "derivation": "constraint",
            "snapshot": { "element": [
                { "id": "Patient", "path": "Patient", "min": 0, "max": "*" },
                { "id": "Patient.name", "path": "Patient.name", "min": 1, "max": "*",
                  "type": [{ "code": "HumanName",
                             "profile": ["http://health.go.ug/fhir/StructureDefinition/missing"] }] }
            ]}
        });
        let error = served
            .load(&[("ug.moh.core@2.0.0".into(), files(&[broken]))])
            .expect_err("does not compile");
        assert!(error.contains("do not compile"), "{error}");
        assert!(lookup(&served), "the previous terminology stays");
    }
}
