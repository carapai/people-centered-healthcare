//! Administrative areas at validation (ADR 0094; #869): every `Library` of type
//! `administrative-areas` in a package (the national core's regions and districts) must be a
//! usable set, and [`Publications::administrative_areas`] reads the newest published one, which
//! the facility registry's NHFR import links facilities to districts by.

use health_configuration::lifecycle::Stage;
use health_facility_registry::AdministrativeAreas;
use health_packages::Packed;
use health_registries_app::places::{administrative_areas, is_administrative_areas};
use serde_json::Value;

use crate::{Error, Publications, failed};

/// The package's administrative areas as written, each with its file, or why it is unreadable.
fn written(packed: &Packed) -> Vec<(String, Result<AdministrativeAreas, String>)> {
    packed
        .contents
        .artefacts
        .iter()
        .filter(|a| a.resource_type == "Library")
        .filter_map(|a| {
            let value = serde_json::from_slice::<Value>(packed.files.get(&a.path)?).ok()?;
            is_administrative_areas(&value).then(|| (a.path.clone(), administrative_areas(&value)))
        })
        .collect()
}

/// Every problem with the package's administrative areas, each prefixed with its file.
pub(crate) fn problems(packed: &Packed) -> Vec<String> {
    written(packed)
        .into_iter()
        .flat_map(|(path, areas)| {
            let problems = match areas {
                Ok(areas) => areas.problems(),
                Err(why) => vec![why],
            };
            problems
                .into_iter()
                .map(move |problem| format!("{path}: {problem}"))
        })
        .collect()
}

impl Publications {
    /// The administrative areas of the newest published package version that holds any, or
    /// none when no published package does.
    pub async fn administrative_areas(&self) -> Result<Option<AdministrativeAreas>, Error> {
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
        // Oldest first, so the newest published version's areas are the last found.
        published.sort_by(|a, b| (&a.version, &a.package).cmp(&(&b.version, &b.package)));
        let mut newest = None;
        for lifecycle in published {
            let packed = self.packed(&lifecycle.content).await?;
            for (_, areas) in written(&packed) {
                if let Ok(areas) = areas {
                    newest = Some(areas);
                }
            }
        }
        Ok(newest)
    }
}
