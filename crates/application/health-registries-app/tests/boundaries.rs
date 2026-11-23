//! Importing a boundary file into the national core package (#604; ADR 0075, ADR 0094): a
//! GeoJSON file naming districts as UBOS does is matched to the package's areas and written as an
//! administrative-boundaries `Library` that reads back the same and passes tier-1 validation.

use health_facility_registry::{AdministrativeAreas, Area, District, import_boundaries};
use health_fhir_validation::validate_structure;
use health_registries_app::places::{
    administrative_boundaries, boundaries_library, boundary_file, is_administrative_boundaries,
};
use serde_json::json;

fn areas() -> AdministrativeAreas {
    AdministrativeAreas {
        national: Some("Ministry of Health".into()),
        regions: vec![Area {
            code: "region-central".into(),
            name: "Central".into(),
        }],
        districts: vec![District {
            code: "district-kampala".into(),
            name: "Kampala".into(),
            region: "region-central".into(),
        }],
    }
}

fn square(x: f64, y: f64, size: f64) -> serde_json::Value {
    json!({
        "type": "Polygon",
        "coordinates": [[[x, y], [x + size, y], [x + size, y + size], [x, y + size], [x, y]]],
    })
}

#[test]
fn a_boundary_file_becomes_the_core_package_s_boundaries_library() {
    let file = json!({
        "type": "FeatureCollection",
        "features": [
            { "type": "Feature", "properties": { "DName2019": "KAMPALA" }, "geometry": square(32.5, 0.25, 0.15) },
            { "type": "Feature", "properties": { "code": "region-central" }, "geometry": square(31.0, -1.0, 2.5) },
        ],
    });
    let features = boundary_file(&file, Some("DName2019")).expect("read");
    let boundaries = import_boundaries(&areas(), features).expect("imported");
    let areas_library = json!({
        "resourceType": "Library",
        "url": "https://health.go.ug/fhir/Library/administrative-areas",
        "version": "1.1.0",
    });
    let library = boundaries_library(&areas_library, &boundaries);
    assert!(is_administrative_boundaries(&library));
    assert_eq!(
        library["url"],
        "https://health.go.ug/fhir/Library/administrative-boundaries"
    );
    assert_eq!(library["version"], "1.1.0");
    assert_eq!(
        administrative_boundaries(&library).expect("read back"),
        boundaries
    );
    let problems = validate_structure(&library.to_string());
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn a_feature_without_a_polygon_or_of_no_area_is_refused() {
    let file = json!({
        "type": "FeatureCollection",
        "features": [
            { "type": "Feature", "properties": { "name": "Kampala" }, "geometry": { "type": "Point", "coordinates": [32.5, 0.3] } },
        ],
    });
    let problems = boundary_file(&file, None).expect_err("refused");
    assert_eq!(
        problems,
        vec!["feature 1 has no Polygon or MultiPolygon geometry"]
    );
    let file = json!({
        "type": "FeatureCollection",
        "features": [
            { "type": "Feature", "properties": { "name": "Lira" }, "geometry": square(32.9, 2.2, 0.3) },
        ],
    });
    let features = boundary_file(&file, None).expect("read");
    let problems = import_boundaries(&areas(), features).expect_err("refused");
    assert_eq!(
        problems,
        vec!["\"Lira\" is not a region or district of the national core"]
    );
    assert!(boundary_file(&json!({ "type": "Feature" }), None).is_err());
}
