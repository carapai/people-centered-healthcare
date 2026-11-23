//! `central-server boundaries <package-dir> <boundary-file> [--name <property>]` (#604; ADR 0075,
//! ADR 0094): imports a boundary file (GeoJSON from UBOS or MoH) into a package directory holding
//! the regions and districts (`Library/administrative-areas`, the national core): each feature is
//! matched to an area by its `code` property, else by its name (the `name` property, or the one
//! given with `--name`), checked against the place hierarchy, and written as
//! `Library-administrative-boundaries.json` beside the areas. Nothing is written when a feature
//! matches no area or two, an area is given twice, a shape is not a closed polygon, or a
//! district lies outside its region. Run it before drafting the package's next version; keep the
//! file's polygons simplified, since every node receives the package.

use std::path::Path;

use health_facility_registry::import_boundaries;
use health_registries_app::places::{
    administrative_areas, boundaries_library, boundary_file, is_administrative_areas,
};
use serde_json::Value;

/// The file the boundaries are written to, in the package directory.
pub(crate) const LIBRARY_FILE: &str = "Library-administrative-boundaries.json";

/// Imports one boundary file; see the module documentation.
pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (Some(package), Some(file)) = (args.first(), args.get(1)) else {
        return Err(
            "usage: central-server boundaries <package-dir> <boundary-file> [--name <property>]"
                .into(),
        );
    };
    let name = match args.get(2..) {
        Some([flag, property]) if flag == "--name" => Some(property.as_str()),
        Some([]) | None => None,
        Some(_) => return Err("the only option is --name <property>".into()),
    };
    let report = import(Path::new(package), Path::new(file), name)?;
    std::io::Write::write_all(&mut std::io::stdout(), format!("{report}\n").as_bytes())?;
    Ok(())
}

/// Imports `file` into `package`, naming areas by `name_property`, and says what was written.
pub(crate) fn import(
    package: &Path,
    file: &Path,
    name_property: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let areas_library = std::fs::read_dir(package)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| serde_json::from_slice::<Value>(&std::fs::read(path).ok()?).ok())
        .find(is_administrative_areas)
        .ok_or("the package holds no regions and districts (Library/administrative-areas)")?;
    let areas = administrative_areas(&areas_library)?;
    let geojson: Value = serde_json::from_slice(&std::fs::read(file)?)
        .map_err(|e| format!("the boundary file is not JSON: {e}"))?;
    let refused = |problems: Vec<String>| format!("nothing written:\n- {}", problems.join("\n- "));
    let features = boundary_file(&geojson, name_property).map_err(refused)?;
    let boundaries = import_boundaries(&areas, features).map_err(refused)?;
    let library = boundaries_library(&areas_library, &boundaries);
    let mut json = serde_json::to_string_pretty(&library)?;
    json.push('\n');
    std::fs::write(package.join(LIBRARY_FILE), json)?;
    let missing = boundaries.missing(&areas);
    Ok(if missing.is_empty() {
        format!(
            "wrote {} boundaries to {LIBRARY_FILE}: every region and district has one",
            boundaries.features.len()
        )
    } else {
        format!(
            "wrote {} boundaries to {LIBRARY_FILE}; none for {}",
            boundaries.features.len(),
            missing.join(", ")
        )
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use health_registries_app::places::administrative_boundaries;

    use super::*;

    fn reference(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../packages/reference-packages")
            .join(path)
    }

    /// The national core's boundaries are its source file imported: importing it again into a
    /// copy of the package writes exactly the Library the package ships.
    #[test]
    fn the_core_package_s_boundaries_are_its_boundary_file_imported() {
        let dir = tempfile::tempdir().expect("dir");
        for file in ["Library-administrative-areas.json", "package.json"] {
            std::fs::copy(reference("core").join(file), dir.path().join(file)).expect("copy");
        }
        let report = import(
            dir.path(),
            &reference("sources/administrative-boundaries.geojson"),
            None,
        )
        .expect("imported");
        assert_eq!(
            report,
            "wrote 11 boundaries to Library-administrative-boundaries.json: every region and district has one"
        );
        let written = std::fs::read_to_string(dir.path().join(LIBRARY_FILE)).expect("written");
        let shipped =
            std::fs::read_to_string(reference("core").join(LIBRARY_FILE)).expect("shipped");
        assert_eq!(written, shipped);
        let library: Value = serde_json::from_str(&shipped).expect("json");
        let boundaries = administrative_boundaries(&library).expect("boundaries");
        assert!(boundaries.of("region-northern").is_some());
        assert!(boundaries.of("district-gulu").is_some());
    }

    #[test]
    fn a_feature_of_no_area_writes_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::copy(
            reference("core/Library-administrative-areas.json"),
            dir.path().join("Library-administrative-areas.json"),
        )
        .expect("copy");
        let file = dir.path().join("boundaries.geojson");
        std::fs::write(
            &file,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"DName2019":"LIRA"},"geometry":{"type":"Polygon","coordinates":[[[32.8,2.2],[33.1,2.2],[33.1,2.5],[32.8,2.2]]]}}]}"#,
        )
        .expect("file");
        let error = import(dir.path(), &file, Some("DName2019")).expect_err("refused");
        assert!(
            error
                .to_string()
                .contains("\"LIRA\" is not a region or district"),
            "{error}"
        );
        assert!(!dir.path().join(LIBRARY_FILE).exists());
    }
}
