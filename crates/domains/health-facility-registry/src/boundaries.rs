//! District and region boundaries (ADR 0075, ADR 0094; #604): the geometry maps shade, shipped
//! in the national core package beside its regions and districts, as a GeoJSON
//! `FeatureCollection` whose every feature is one area's (Multi)Polygon, named by its code.
//!
//! - [`import_boundaries`] reads a boundary file (UBOS/MoH geometry, each feature named by an
//!   area's code or its name) against the package's [`AdministrativeAreas`], and refuses a
//!   feature that matches no area, or one area twice.
//! - [`AdministrativeBoundaries::problems`] is what validation refuses: a code that is not a
//!   region or district of the package, a code twice, a ring that is not closed or out of range,
//!   and a district lying outside its region's boundary (checked against the place hierarchy).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::areas::AdministrativeAreas;
use crate::place::PlaceLevel;

/// The `Library.type` code of a package's area boundaries.
pub const BOUNDARIES_TYPE: &str = "administrative-boundaries";
/// The content type of the boundaries' attachment: GeoJSON (RFC 7946).
pub const BOUNDARIES_CONTENT: &str = "application/geo+json";

/// A linear ring: `[longitude, latitude]` positions, the last the same as the first.
pub type Ring = Vec<[f64; 2]>;

/// An area's shape: one polygon, or several (a district with islands). Each polygon is its outer
/// ring, then any holes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "coordinates")]
pub enum Geometry {
    /// One polygon.
    Polygon(Vec<Ring>),
    /// Several polygons.
    MultiPolygon(Vec<Vec<Ring>>),
}

impl Geometry {
    /// Its polygons, each outer ring first.
    pub fn polygons(&self) -> Vec<&[Ring]> {
        match self {
            Self::Polygon(rings) => vec![rings.as_slice()],
            Self::MultiPolygon(polygons) => polygons.iter().map(Vec::as_slice).collect(),
        }
    }

    /// Whether `point` (`[longitude, latitude]`) lies inside it: inside an outer ring and in none
    /// of that polygon's holes.
    pub fn contains(&self, point: [f64; 2]) -> bool {
        self.polygons().into_iter().any(|rings| {
            let mut rings = rings.iter();
            rings.next().is_some_and(|outer| in_ring(outer, point))
                && !rings.any(|hole| in_ring(hole, point))
        })
    }

    /// The mean of its outer rings' positions (the closing one left out): a point a district's
    /// boundary is placed in its region by.
    pub fn centre(&self) -> Option<[f64; 2]> {
        let positions: Vec<&[f64; 2]> = self
            .polygons()
            .into_iter()
            .filter_map(|rings| rings.first())
            .flat_map(|outer| outer.iter().skip(1))
            .collect();
        if positions.is_empty() {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        let count = positions.len() as f64;
        let sum = positions
            .iter()
            .fold([0.0, 0.0], |[x, y], p| [x + p[0], y + p[1]]);
        Some([sum[0] / count, sum[1] / count])
    }

    /// Why it is not a usable shape: no polygon, a ring of fewer than four positions or not
    /// closed, a position out of range.
    fn problems(&self) -> Vec<String> {
        let polygons = self.polygons();
        if polygons.iter().all(|rings| rings.is_empty()) {
            return vec!["has no polygon".to_owned()];
        }
        let mut problems = Vec::new();
        for ring in polygons.into_iter().flatten() {
            if ring.len() < 4 {
                problems.push("has a ring of fewer than four positions".to_owned());
            } else if ring.first() != ring.last() {
                problems.push("has a ring that is not closed".to_owned());
            }
            if ring
                .iter()
                .any(|[x, y]| !x.is_finite() || !y.is_finite() || x.abs() > 180.0 || y.abs() > 90.0)
            {
                problems.push("has a position out of range".to_owned());
            }
        }
        problems.dedup();
        problems
    }
}

/// Ray casting: whether `point` is inside `ring`.
fn in_ring(ring: &[[f64; 2]], [x, y]: [f64; 2]) -> bool {
    let mut inside = false;
    for edge in ring.windows(2) {
        let ([x1, y1], [x2, y2]) = (edge[0], edge[1]);
        if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
            inside = !inside;
        }
    }
    inside
}

/// What a boundary feature says of its area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryProperties {
    /// The region's or district's code in the package's [`AdministrativeAreas`].
    pub code: String,
    /// Its name, as the areas give it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One area's boundary: a GeoJSON `Feature`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename = "Feature")]
pub struct Boundary {
    /// Whose boundary it is.
    pub properties: BoundaryProperties,
    /// Its shape.
    pub geometry: Geometry,
}

/// The regions' and districts' boundaries, as the national core package holds them: a GeoJSON
/// `FeatureCollection`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename = "FeatureCollection")]
pub struct AdministrativeBoundaries {
    /// One per area that has a boundary.
    pub features: Vec<Boundary>,
}

impl AdministrativeBoundaries {
    /// The boundary of the area with `code`.
    pub fn of(&self, code: &str) -> Option<&Boundary> {
        self.features.iter().find(|b| b.properties.code == code)
    }

    /// Every reason these are not usable boundaries of `areas`: a code that is not one of its
    /// regions or districts, a code twice, a shape that is not a closed polygon in range, and a
    /// district whose boundary lies outside its region's.
    pub fn problems(&self, areas: &AdministrativeAreas) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = BTreeSet::new();
        for boundary in &self.features {
            let code = &boundary.properties.code;
            if level_of(areas, code).is_none() {
                problems.push(format!(
                    "the boundary {code:?} is not a region or district of the national core"
                ));
            }
            if !seen.insert(code.as_str()) {
                problems.push(format!("the boundary {code:?} is given twice"));
            }
            for problem in boundary.geometry.problems() {
                problems.push(format!("the boundary {code:?} {problem}"));
            }
        }
        for district in &areas.districts {
            let (Some(inner), Some(outer)) = (self.of(&district.code), self.of(&district.region))
            else {
                continue;
            };
            if let Some(centre) = inner.geometry.centre()
                && !outer.geometry.contains(centre)
            {
                problems.push(format!(
                    "the district {:?} lies outside its region {:?}",
                    district.code, district.region
                ));
            }
        }
        problems
    }

    /// The regions and districts of `areas` with no boundary here, regions first.
    pub fn missing<'a>(&self, areas: &'a AdministrativeAreas) -> Vec<&'a str> {
        areas
            .regions
            .iter()
            .map(|r| r.code.as_str())
            .chain(areas.districts.iter().map(|d| d.code.as_str()))
            .filter(|code| self.of(code).is_none())
            .collect()
    }
}

/// Whether `code` is a region or a district of `areas`.
fn level_of(areas: &AdministrativeAreas, code: &str) -> Option<PlaceLevel> {
    if areas.regions.iter().any(|r| r.code == code) {
        Some(PlaceLevel::Region)
    } else if areas.district(code).is_some() {
        Some(PlaceLevel::District)
    } else {
        None
    }
}

/// One feature of a boundary file to import: its shape, and the area it names, by code or by
/// name.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryFeature {
    /// The area's code, when the file gives one.
    pub code: Option<String>,
    /// The area's name, when the file names it instead (UBOS names districts).
    pub name: Option<String>,
    /// Its shape.
    pub geometry: Geometry,
}

/// A name compared loosely: case and spacing ignored, and a trailing ` district`, ` region` or
/// ` city` dropped, as boundary files write them.
fn loose(name: &str) -> String {
    let name = name.trim().to_lowercase();
    ["district", "region", "city"]
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or(&name)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The boundaries of `areas` read from a boundary file's `features`: each matched to a region or
/// district by its code, else by its name (which must name exactly one area), and written with
/// the area's code and name. Refused, with every reason, when a feature matches no area or two,
/// an area is matched twice, or the result has any of [`AdministrativeBoundaries::problems`].
pub fn import_boundaries(
    areas: &AdministrativeAreas,
    features: Vec<BoundaryFeature>,
) -> Result<AdministrativeBoundaries, Vec<String>> {
    let named: Vec<(&str, &str)> = areas
        .regions
        .iter()
        .map(|r| (r.code.as_str(), r.name.as_str()))
        .chain(
            areas
                .districts
                .iter()
                .map(|d| (d.code.as_str(), d.name.as_str())),
        )
        .collect();
    let mut problems = Vec::new();
    let mut boundaries = AdministrativeBoundaries::default();
    for (index, feature) in features.into_iter().enumerate() {
        let matched: Vec<&(&str, &str)> = match (&feature.code, &feature.name) {
            (Some(code), _) => named.iter().filter(|(c, _)| c == code).collect(),
            (None, Some(name)) => named
                .iter()
                .filter(|(_, n)| loose(n) == loose(name))
                .collect(),
            (None, None) => Vec::new(),
        };
        let what = feature
            .code
            .as_deref()
            .or(feature.name.as_deref())
            .map_or_else(|| format!("feature {}", index + 1), |w| format!("{w:?}"));
        match matched.as_slice() {
            [(code, name)] => boundaries.features.push(Boundary {
                properties: BoundaryProperties {
                    code: (*code).to_owned(),
                    name: Some((*name).to_owned()),
                },
                geometry: feature.geometry,
            }),
            [] => problems.push(format!(
                "{what} is not a region or district of the national core"
            )),
            _ => problems.push(format!("{what} names more than one area")),
        }
    }
    problems.extend(boundaries.problems(areas));
    if problems.is_empty() {
        Ok(boundaries)
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Area, District};

    fn areas() -> AdministrativeAreas {
        AdministrativeAreas {
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
                    code: "district-wakiso".into(),
                    name: "Wakiso".into(),
                    region: "region-central".into(),
                },
            ],
        }
    }

    /// A square from `(x, y)` with sides `size`.
    fn square(x: f64, y: f64, size: f64) -> Geometry {
        Geometry::Polygon(vec![vec![
            [x, y],
            [x + size, y],
            [x + size, y + size],
            [x, y + size],
            [x, y],
        ]])
    }

    fn feature(code: Option<&str>, name: Option<&str>, geometry: Geometry) -> BoundaryFeature {
        BoundaryFeature {
            code: code.map(str::to_owned),
            name: name.map(str::to_owned),
            geometry,
        }
    }

    #[test]
    fn importing_matches_features_to_areas_by_code_or_name() {
        let imported = import_boundaries(
            &areas(),
            vec![
                feature(Some("region-central"), None, square(31.0, -1.0, 3.0)),
                feature(None, Some("KAMPALA District"), square(32.5, 0.2, 0.2)),
            ],
        )
        .expect("imported");
        assert_eq!(imported.features.len(), 2);
        let kampala = imported.of("district-kampala").expect("kampala");
        assert_eq!(kampala.properties.name.as_deref(), Some("Kampala"));
        assert_eq!(imported.missing(&areas()), vec!["district-wakiso"]);
    }

    #[test]
    fn importing_refuses_a_feature_of_no_area_or_an_area_twice() {
        let problems = import_boundaries(
            &areas(),
            vec![
                feature(None, Some("Gulu"), square(32.0, 2.5, 0.5)),
                feature(Some("district-kampala"), None, square(32.5, 0.2, 0.2)),
                feature(None, Some("Kampala"), square(32.5, 0.2, 0.2)),
                feature(None, None, square(32.5, 0.2, 0.2)),
            ],
        )
        .expect_err("refused");
        assert!(
            problems.iter().any(|p| p.contains("\"Gulu\" is not")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("feature 4")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("given twice")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_district_outside_its_region_is_refused_against_the_hierarchy() {
        let boundaries = AdministrativeBoundaries {
            features: vec![
                Boundary {
                    properties: BoundaryProperties {
                        code: "region-central".into(),
                        name: None,
                    },
                    geometry: square(31.0, -1.0, 3.0),
                },
                Boundary {
                    properties: BoundaryProperties {
                        code: "district-wakiso".into(),
                        name: None,
                    },
                    geometry: square(36.0, 0.0, 0.5),
                },
            ],
        };
        let problems = boundaries.problems(&areas());
        assert_eq!(
            problems,
            vec!["the district \"district-wakiso\" lies outside its region \"region-central\""]
        );
    }

    #[test]
    fn shapes_must_be_closed_rings_in_range() {
        let boundaries = AdministrativeBoundaries {
            features: vec![
                Boundary {
                    properties: BoundaryProperties {
                        code: "district-kampala".into(),
                        name: None,
                    },
                    geometry: Geometry::Polygon(vec![vec![
                        [32.5, 0.2],
                        [32.7, 0.2],
                        [32.7, 0.4],
                        [32.5, 0.4],
                    ]]),
                },
                Boundary {
                    properties: BoundaryProperties {
                        code: "district-wakiso".into(),
                        name: None,
                    },
                    geometry: Geometry::MultiPolygon(vec![vec![vec![
                        [200.0, 0.0],
                        [1.0, 0.0],
                        [1.0, 1.0],
                        [200.0, 0.0],
                    ]]]),
                },
            ],
        };
        let problems = boundaries.problems(&areas());
        assert!(
            problems.iter().any(|p| p.contains("not closed")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("out of range")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_point_is_inside_an_outer_ring_and_outside_its_holes() {
        let ring = |x: f64, size: f64| {
            vec![
                [x, x],
                [x + size, x],
                [x + size, x + size],
                [x, x + size],
                [x, x],
            ]
        };
        let holed = Geometry::Polygon(vec![ring(0.0, 4.0), ring(1.0, 2.0)]);
        assert!(holed.contains([0.5, 0.5]));
        assert!(!holed.contains([2.0, 2.0]));
        assert!(!holed.contains([5.0, 5.0]));
        assert_eq!(square(0.0, 0.0, 2.0).centre(), Some([1.0, 1.0]));
    }

    #[test]
    fn boundaries_read_and_write_as_geojson() {
        let boundaries = AdministrativeBoundaries {
            features: vec![Boundary {
                properties: BoundaryProperties {
                    code: "district-kampala".into(),
                    name: Some("Kampala".into()),
                },
                geometry: square(32.5, 0.2, 0.25),
            }],
        };
        let written = serde_json::to_string(&boundaries).expect("written");
        assert!(
            written.starts_with("{\"type\":\"FeatureCollection\""),
            "{written}"
        );
        assert!(written.contains("\"type\":\"Feature\""), "{written}");
        assert!(
            written.contains("\"type\":\"Polygon\",\"coordinates\":[[[32.5,0.2]"),
            "{written}"
        );
    }
}
