//! Tiers 3–4 on the write path (#91; fhir-r5-model.md § Validation; ADRs 0016, 0053).
//!
//! A resource that passed tiers 1–2 is validated against:
//! - every profile it claims in `meta.profile`, which must be active on the node;
//! - every profile the active configuration requires of its type.
//!
//! Bindings are checked through the node's [`ActiveTerminology`]. Errors refuse the write with
//! an OperationOutcome that also lists any warnings; warnings alone let it through.
//!
//! Activation (#89) sets what is active, compiling the profiles once, and replaces it when the
//! active packages change. Until a node has [`ActiveProfiles`], tiers 3–4 do not run.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use health_fhir_validation::{
    Bindings, FhirPathEngine, Membership, ProfileSet, ProfileValidator, Severity, SnapshotValidator,
};
use health_terminology::{Error, ExpandRequest, Validation};
use serde_json::Value;

use crate::terminology::ActiveTerminology;
use crate::{FhirError, FhirServer};

/// The profiles active on a node: compiled once, and the ones each resource type must meet.
#[derive(Debug, Clone, Default)]
pub struct Profiles {
    /// Every active profile, compiled.
    pub set: Arc<ProfileSet>,
    /// The profiles every resource of a type must meet, by type, e.g. the national core
    /// Patient profile.
    pub required: BTreeMap<String, Vec<String>>,
}

/// The profiles a node validates writes against. Activation (#89) replaces them; a write keeps
/// the set it started with.
#[derive(Debug, Clone, Default)]
pub struct ActiveProfiles(Arc<RwLock<Arc<Profiles>>>);

impl ActiveProfiles {
    /// Validating against `profiles`.
    pub fn new(profiles: Profiles) -> Self {
        Self(Arc::new(RwLock::new(Arc::new(profiles))))
    }

    /// What is active now.
    pub fn current(&self) -> Arc<Profiles> {
        self.0.read().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |guard| guard.clone(),
        )
    }

    /// Validates against `profiles` from now on.
    pub fn replace(&self, profiles: Profiles) {
        let next = Arc::new(profiles);
        match self.0.write() {
            Ok(mut guard) => *guard = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }
}

/// [`Bindings`] answered by the node's terminology: a pinned expansion where the value set
/// ships one, otherwise its compose over local code systems.
struct TerminologyBindings(ActiveTerminology);

impl Bindings for TerminologyBindings {
    fn member(&self, value_set: &str, system: Option<&str>, code: &str) -> Membership {
        let terminology = self.0.current();
        let unknown = |error: Error| Membership::Unknown(error.to_string());
        match system {
            Some(system) => match terminology.validate_code(value_set, system, code, None) {
                Ok(Validation::Valid { .. } | Validation::WrongDisplay { .. }) => Membership::In,
                Ok(Validation::Invalid) => Membership::NotIn,
                Err(error) => unknown(error),
            },
            // A bare `code` element names no system: any of the value set's systems will do.
            None => match terminology.expand(value_set, &ExpandRequest::default()) {
                Ok(expansion) if expansion.contains.iter().any(|c| c.code == code) => {
                    Membership::In
                }
                Ok(_) => Membership::NotIn,
                Err(error) => unknown(error),
            },
        }
    }
}

/// The profiles `resource` claims in `meta.profile`, and those its type must meet.
fn profiles_for(resource: &Value, profiles: &Profiles) -> Vec<String> {
    let mut found: Vec<String> = resource["meta"]["profile"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if let Some(required) = resource["resourceType"]
        .as_str()
        .and_then(|kind| profiles.required.get(kind))
    {
        for url in required {
            if !found
                .iter()
                .any(|claimed| claimed.split('|').next() == Some(url))
            {
                found.push(url.clone());
            }
        }
    }
    found
}

/// Tiers 3–4 for `resource`, when the server has active profiles: 400 on any error.
pub(crate) async fn check(
    server: &FhirServer,
    engine: Arc<dyn FhirPathEngine>,
    resource: &Value,
) -> Result<(), FhirError> {
    let Some(active) = &server.profiles else {
        return Ok(());
    };
    let profiles = active.current();
    let urls = profiles_for(resource, &profiles);
    if urls.is_empty() {
        return Ok(());
    }
    let mut validator = SnapshotValidator::new(profiles.set.clone(), engine);
    if let Some(terminology) = &server.terminology {
        validator = validator.with_bindings(Arc::new(TerminologyBindings(terminology.clone())));
    }
    let found = validator.validate(resource, &urls).await;
    if found.iter().any(|issue| issue.severity == Severity::Error) {
        Err(FhirError::invalid_resource(&found))
    } else {
        Ok(())
    }
}
