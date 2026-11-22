use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::place::{NATIONAL, Place, PlaceLevel};

/// The `Library.type` code of a package's administrative areas (ADR 0094: districts and regions
/// ship in the national core package).
pub const AREAS_TYPE: &str = "administrative-areas";
/// The content type of the areas' attachment.
pub const AREAS_CONTENT: &str = "application/vnd.health.administrative-areas+json";

/// A region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    /// Its code.
    pub code: String,
    /// Its name.
    pub name: String,
}

/// A district, in its region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct District {
    /// Its code: what the NHFR's facility list names a facility's district by.
    pub code: String,
    /// Its name.
    pub name: String,
    /// Its region's code.
    pub region: String,
}

/// The country's regions and districts, as the national core package holds them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdministrativeAreas {
    /// The national place's name (the Ministry of Health).
    #[serde(default)]
    pub national: Option<String>,
    /// The regions.
    #[serde(default)]
    pub regions: Vec<Area>,
    /// The districts.
    #[serde(default)]
    pub districts: Vec<District>,
}

impl AdministrativeAreas {
    /// Every reason these are not a usable set of areas: a code used twice or reserved, a
    /// district in no region the set holds, an empty name.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut codes = BTreeSet::new();
        let regions: BTreeSet<&str> = self.regions.iter().map(|r| r.code.as_str()).collect();
        let named = self
            .regions
            .iter()
            .map(|r| (&r.code, &r.name))
            .chain(self.districts.iter().map(|d| (&d.code, &d.name)));
        for (code, name) in named {
            if code.trim().is_empty() {
                problems.push("an area has no code".to_owned());
            } else if code == NATIONAL {
                problems.push(format!("{code:?} is the national place's code"));
            } else if !codes.insert(code.as_str()) {
                problems.push(format!("the code {code:?} is used twice"));
            }
            if name.trim().is_empty() {
                problems.push(format!("{code:?} has no name"));
            }
        }
        for district in &self.districts {
            if !regions.contains(district.region.as_str()) {
                problems.push(format!(
                    "the district {:?} is in the region {:?}, which is not listed",
                    district.code, district.region
                ));
            }
        }
        problems
    }

    /// The district with `code`.
    pub fn district(&self, code: &str) -> Option<&District> {
        self.districts.iter().find(|d| d.code == code)
    }

    /// The national place, the regions and the districts as places.
    pub fn places(&self) -> Vec<Place> {
        let area = |code: &str, name: &str, level, parent: &str| Place {
            code: code.to_owned(),
            name: name.to_owned(),
            level,
            parent: Some(parent.to_owned()),
            active: true,
            care_level: None,
            ownership: None,
            coordinates: None,
        };
        let mut places = vec![Place::national(
            self.national.as_deref().unwrap_or("National"),
        )];
        places.extend(
            self.regions
                .iter()
                .map(|r| area(&r.code, &r.name, PlaceLevel::Region, NATIONAL)),
        );
        places.extend(
            self.districts
                .iter()
                .map(|d| area(&d.code, &d.name, PlaceLevel::District, &d.region)),
        );
        places
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_need_unique_codes_and_listed_regions() {
        let areas = AdministrativeAreas {
            national: Some("Ministry of Health".into()),
            regions: vec![Area {
                code: "region-central".into(),
                name: "Central".into(),
            }],
            districts: vec![
                District {
                    code: "district-kampala".into(),
                    name: "Kampala".into(),
                    region: "region-central".into(),
                },
                District {
                    code: "district-kampala".into(),
                    name: "Kampala again".into(),
                    region: "region-north".into(),
                },
                District {
                    code: NATIONAL.into(),
                    name: String::new(),
                    region: "region-central".into(),
                },
            ],
        };
        let problems = areas.problems();
        assert!(
            problems.iter().any(|p| p.contains("used twice")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("not listed")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("national")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("no name")),
            "{problems:?}"
        );
        let places = areas.places();
        assert_eq!(places[0].code, NATIONAL);
        assert_eq!(places[1].parent.as_deref(), Some(NATIONAL));
        assert_eq!(places[2].parent.as_deref(), Some("region-central"));
    }
}
