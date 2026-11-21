//! Age Band Sets at validation (analytics.md § Age Band Sets and Population Estimates; ADR 0071;
//! #551): every `Library` of type `age-band-set` in a package, the national core's or a
//! program's, must be a usable set, and a gap or an overlap between its bands fails. Changing a
//! band needs a major version and a rename a patch: the semver check reads the same content
//! (`health_packages::bump`).
//!
//! [`Publications::age_band_sets`] lists every version of every set ever published, so an
//! indicator pinned to `url|version` keeps that version's bands after a later one is published.

use health_analytics::AgeBandSet;
use health_analytics_app::{age_band_set, is_age_band_set};
use health_configuration::lifecycle::Stage;
use health_packages::Packed;
use serde_json::Value;

use crate::{Error, Publications, failed, lifecycle_id};

/// The package's Age Band Sets as written, each with its file.
fn written(packed: &Packed) -> Vec<(String, health_analytics::AgeBandSetDefinition)> {
    packed
        .contents
        .artefacts
        .iter()
        .filter(|a| a.resource_type == "Library")
        .filter_map(|a| {
            let value = serde_json::from_slice::<Value>(packed.files.get(&a.path)?).ok()?;
            is_age_band_set(&value).then(|| (a.path.clone(), age_band_set(&value)))
        })
        .collect()
}

/// Every problem with the package's Age Band Sets, each prefixed with its file.
pub(crate) fn problems(packed: &Packed) -> Vec<String> {
    written(packed)
        .into_iter()
        .flat_map(|(path, set)| {
            set.problems()
                .into_iter()
                .map(move |problem| format!("{path}: {problem}"))
        })
        .collect()
}

/// The package's usable Age Band Sets.
pub fn age_band_sets_in(packed: &Packed) -> Vec<AgeBandSet> {
    written(packed)
        .into_iter()
        .filter_map(|(_, set)| set.age_band_set().ok())
        .collect()
}

/// An Age Band Set as published, with the package version that published it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedAgeBandSet {
    /// The package version, `name@version`.
    pub package: String,
    /// The set.
    pub set: AgeBandSet,
}

impl Publications {
    /// Every version of every Age Band Set published, by package version, oldest first: what
    /// an indicator's pinned `url|version` is looked up in.
    pub async fn age_band_sets(&self) -> Result<Vec<PublishedAgeBandSet>, Error> {
        let mut tx = self.begin().await?;
        let mut published = Vec::new();
        for json in tx.package_lifecycles().await.map_err(failed)? {
            let lifecycle: health_configuration::lifecycle::Lifecycle =
                serde_json::from_str(&json).map_err(failed)?;
            if lifecycle.stage == Stage::Published {
                published.push(lifecycle);
            }
        }
        drop(tx);
        published.sort_by(|a, b| (&a.package, &a.version).cmp(&(&b.package, &b.version)));
        let mut found = Vec::new();
        for lifecycle in published {
            let packed = self.packed(&lifecycle.content).await?;
            let package = lifecycle_id(&lifecycle);
            found.extend(
                age_band_sets_in(&packed)
                    .into_iter()
                    .map(|set| PublishedAgeBandSet {
                        package: package.clone(),
                        set,
                    }),
            );
        }
        Ok(found)
    }
}
