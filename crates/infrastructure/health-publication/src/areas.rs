//! Administrative areas at validation (ADR 0094; #869): every `Library` of type
//! `administrative-areas` in a package (the national core's regions and districts) must be a
//! usable set, and [`Publications::administrative_areas`] reads the newest published one, which
//! the facility registry's NHFR import links facilities to districts by.
//!
//! Their boundaries (#604): a `Library` of type `administrative-boundaries` must sit beside the
//! areas in the same package and be usable boundaries of them (each a region or district, once,
//! a closed polygon, a district inside its region), and
//! [`Publications::administrative_boundaries`] reads the newest published one, which maps draw.

use health_configuration::lifecycle::Stage;
use health_facility_registry::{AdministrativeAreas, AdministrativeBoundaries};
use health_packages::Packed;
use health_registries_app::places::{
    administrative_areas, administrative_boundaries, is_administrative_areas,
    is_administrative_boundaries,
};
use serde_json::Value;

use crate::{Error, Publications, failed};

/// Each Library found in a package, with its file, or why it is unreadable.
type Found<T> = Vec<(String, Result<T, String>)>;

/// The package's Libraries that `is` picks, read by `read`, each with its file, or why it is
/// unreadable.
fn libraries<T>(
    packed: &Packed,
    is: fn(&Value) -> bool,
    read: fn(&Value) -> Result<T, String>,
) -> Found<T> {
    packed
        .contents
        .artefacts
        .iter()
        .filter(|a| a.resource_type == "Library")
        .filter_map(|a| {
            let value = serde_json::from_slice::<Value>(packed.files.get(&a.path)?).ok()?;
            is(&value).then(|| (a.path.clone(), read(&value)))
        })
        .collect()
}

/// The package's administrative areas as written, each with its file, or why it is unreadable.
fn written(packed: &Packed) -> Found<AdministrativeAreas> {
    libraries(packed, is_administrative_areas, administrative_areas)
}

/// The package's area boundaries as written, each with its file, or why it is unreadable.
fn boundaries(packed: &Packed) -> Found<AdministrativeBoundaries> {
    libraries(
        packed,
        is_administrative_boundaries,
        administrative_boundaries,
    )
}

/// Every problem with the package's administrative areas and their boundaries, each prefixed
/// with its file. Boundaries are checked against the areas of the same package.
pub(crate) fn problems(packed: &Packed) -> Vec<String> {
    let areas = written(packed);
    let mut problems: Vec<String> = areas
        .iter()
        .flat_map(|(path, areas)| {
            let problems = match areas {
                Ok(areas) => areas.problems(),
                Err(why) => vec![why.clone()],
            };
            problems
                .into_iter()
                .map(move |problem| format!("{path}: {problem}"))
        })
        .collect();
    let usable = areas.iter().find_map(|(_, a)| a.as_ref().ok());
    for (path, boundaries) in boundaries(packed) {
        let found = match (boundaries, usable) {
            (Err(why), _) => vec![why],
            (Ok(_), None) => vec![
                "boundaries need the regions and districts (administrative-areas) in the same package"
                    .to_owned(),
            ],
            (Ok(boundaries), Some(areas)) => boundaries.problems(areas),
        };
        problems.extend(found.into_iter().map(|p| format!("{path}: {p}")));
    }
    problems
}

impl Publications {
    /// What `read` finds in the newest published package version that holds any, or none when
    /// no published package does.
    async fn newest<T>(&self, read: fn(&Packed) -> Found<T>) -> Result<Option<T>, Error> {
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
        // Oldest first, so the newest published version's is the last found.
        published.sort_by(|a, b| (&a.version, &a.package).cmp(&(&b.version, &b.package)));
        let mut newest = None;
        for lifecycle in published {
            let packed = self.packed(&lifecycle.content).await?;
            for (_, found) in read(&packed) {
                if let Ok(found) = found {
                    newest = Some(found);
                }
            }
        }
        Ok(newest)
    }

    /// The administrative areas of the newest published package version that holds any, or
    /// none when no published package does.
    pub async fn administrative_areas(&self) -> Result<Option<AdministrativeAreas>, Error> {
        self.newest(written).await
    }

    /// The area boundaries of the newest published package version that holds any (#604), or
    /// none when no published package does.
    pub async fn administrative_boundaries(
        &self,
    ) -> Result<Option<AdministrativeBoundaries>, Error> {
        self.newest(boundaries).await
    }
}
