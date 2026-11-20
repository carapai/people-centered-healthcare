//! Package Validation's snapshot check (ADR 0053; #520): each differential element appears in
//! the snapshot with the same constraints, so a stale or hand-edited snapshot cannot be
//! published. The official R5 profiles, our slicing fixtures and the core package's profiles
//! pass unchanged; each kind of drift is named by element and constraint.

use std::path::{Path, PathBuf};

use health_fhir_validation::{SnapshotProblem, check_snapshot};
use serde_json::{Value, json};

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

fn definitions(dir: &Path) -> Vec<(PathBuf, Value)> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("dir")
        .map(|e| e.expect("entry").path())
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let value = read(&path);
            (path, value)
        })
        .filter(|(_, value)| value["resourceType"] == "StructureDefinition")
        .collect()
}

fn fixtures(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir)
}

fn core_package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/fhir-core-package/package")
}

#[test]
fn generated_snapshots_match_their_differentials() {
    let mut checked = 0;
    for dir in [
        fixtures("profiles"),
        fixtures("custom-profiles"),
        core_package(),
    ] {
        for (path, definition) in definitions(&dir) {
            assert_eq!(
                check_snapshot(&definition),
                Vec::new(),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 5 + 3 + 33, "every profile was checked");
}

fn profile(file: &str) -> Value {
    read(&fixtures(file))
}

fn snapshot_element<'a>(definition: &'a mut Value, id: &str) -> &'a mut Value {
    definition["snapshot"]["element"]
        .as_array_mut()
        .expect("snapshot")
        .iter_mut()
        .find(|e| e["id"] == id)
        .expect("element")
}

fn differs(element: &str, constraint: &str) -> SnapshotProblem {
    SnapshotProblem::Differs {
        element: element.to_owned(),
        constraint: constraint.to_owned(),
    }
}

/// `file`'s snapshot with `edit` applied to `id`.
fn edited(file: &str, id: &str, edit: impl FnOnce(&mut Value)) -> Vec<SnapshotProblem> {
    let mut definition = profile(file);
    edit(snapshot_element(&mut definition, id));
    check_snapshot(&definition)
}

#[test]
fn a_snapshot_missing_a_differential_min_names_the_element() {
    let problems = edited("custom-profiles/ug-address.json", "Address.district", |e| {
        e["min"] = json!(0);
    });
    assert_eq!(problems, vec![differs("Address.district", "min")]);
    assert_eq!(
        problems[0].to_string(),
        "Address.district: the snapshot's min is not the differential's"
    );
}

const BP: &str = "profiles/StructureDefinition-bp.json";
const VITAL_SIGNS: &str = "profiles/StructureDefinition-vitalsigns.json";
const UG_PATIENT: &str = "custom-profiles/ug-patient.json";

#[test]
fn each_constraint_is_compared() {
    type Edit = fn(&mut Value);
    let cases: [(&str, &str, &str, Edit); 10] = [
        (BP, "Observation.code.coding:BPCode", "max", |e| {
            e["max"] = json!("*");
        }),
        (
            BP,
            "Observation.code.coding:BPCode.code",
            "fixedCode",
            |e| {
                e["fixedCode"] = json!("0000-0");
            },
        ),
        (
            BP,
            "Observation.code.coding:BPCode.code",
            "fixedCode",
            |e| {
                e.as_object_mut().expect("element").remove("fixedCode");
            },
        ),
        (BP, "Observation.component", "slicing", |e| {
            e["slicing"]["rules"] = json!("closed");
        }),
        (
            UG_PATIENT,
            "Patient.maritalStatus",
            "patternCodeableConcept",
            |e| {
                e["patternCodeableConcept"]["coding"][0]["code"] = json!("X");
            },
        ),
        (VITAL_SIGNS, "Observation.subject", "type", |e| {
            e["type"][0]["targetProfile"] =
                json!(["http://hl7.org/fhir/StructureDefinition/Group"]);
        }),
        (VITAL_SIGNS, "Observation.effective[x]", "type", |e| {
            e["type"] = json!([{ "code": "dateTime" }]);
        }),
        // dateTime may be left out (its type slice is closed), boolean may not.
        (UG_PATIENT, "Patient.deceased[x]", "type", |e| {
            e["type"] = json!([{ "code": "dateTime" }]);
        }),
        (VITAL_SIGNS, "Observation.status", "binding", |e| {
            e["binding"]["strength"] = json!("example");
        }),
        (
            VITAL_SIGNS,
            "Observation.effective[x]",
            "constraint vs-1",
            |e| {
                e["constraint"]
                    .as_array_mut()
                    .expect("constraints")
                    .retain(|c| c["key"] != "vs-1");
            },
        ),
    ];
    for (file, id, constraint, edit) in cases {
        assert_eq!(
            edited(file, id, edit),
            vec![differs(id, constraint)],
            "{file} {id} {constraint}"
        );
    }
}

#[test]
fn a_changed_invariant_expression_is_named() {
    let problems = edited(VITAL_SIGNS, "Observation.effective[x]", |e| {
        for c in e["constraint"].as_array_mut().expect("constraints") {
            if c["key"] == "vs-1" {
                c["expression"] = json!("true");
            }
        }
    });
    assert_eq!(
        problems,
        vec![differs("Observation.effective[x]", "constraint vs-1")]
    );
}

#[test]
fn an_extension_target_profile_is_compared() {
    let mut extension = read(&core_package().join("StructureDefinition-care-plan-author.json"));
    let value = "Extension.value[x]";
    snapshot_element(&mut extension, value)["type"][0]["targetProfile"] =
        json!(["http://hl7.org/fhir/StructureDefinition/Patient"]);
    assert_eq!(check_snapshot(&extension), vec![differs(value, "type")]);
}

#[test]
fn an_element_missing_from_the_snapshot_is_named() {
    let mut definition = profile(BP);
    definition["snapshot"]["element"]
        .as_array_mut()
        .expect("snapshot")
        .retain(|e| e["id"] != "Observation.component:DiastolicBP");
    assert!(
        check_snapshot(&definition).contains(&SnapshotProblem::Missing {
            element: "Observation.component:DiastolicBP".into()
        })
    );
}

#[test]
fn a_renamed_choice_in_the_differential_finds_its_snapshot_slice() {
    // bp's differential says Observation.component:SystolicBP.valueQuantity.code (the R4
    // spelling); its snapshot says ...value[x]:valueQuantity.code.
    let problems = edited(
        BP,
        "Observation.component:SystolicBP.value[x]:valueQuantity.code",
        |e| e["fixedCode"] = json!("mm"),
    );
    assert_eq!(
        problems,
        vec![differs(
            "Observation.component:SystolicBP.valueQuantity.code",
            "fixedCode"
        )]
    );
}

#[test]
fn a_differential_only_profile_has_no_snapshot() {
    let definition = profile("diff-only-profile.json");
    assert_eq!(
        check_snapshot(&definition),
        vec![SnapshotProblem::NoSnapshot]
    );
}

#[test]
fn a_definition_without_a_differential_is_not_checked() {
    let mut definition = profile(BP);
    definition
        .as_object_mut()
        .expect("object")
        .remove("differential");
    assert_eq!(check_snapshot(&definition), Vec::new());
}

#[test]
fn what_the_hl7_generators_write_is_accepted() {
    // A choice narrowed to one type keeps `value[x]` with no type slice, and its open slicing
    // by type is closed (hl7.fhir.uv.extensions.r5's quantity-accuracy and confidential).
    let extension = json!({
        "resourceType": "StructureDefinition",
        "derivation": "constraint",
        "differential": { "element": [
            { "id": "Extension.value[x]", "path": "Extension.value[x]",
              "slicing": { "discriminator": [{ "type": "type", "path": "$this" }], "rules": "open" },
              "type": [{ "code": "Quantity" }] },
            { "id": "Extension.valueQuantity.value", "path": "Extension.valueQuantity.value", "min": 1 }
        ] },
        "snapshot": { "element": [
            { "id": "Extension", "path": "Extension" },
            { "id": "Extension.value[x]", "path": "Extension.value[x]",
              "slicing": { "discriminator": [{ "type": "type", "path": "$this" }], "ordered": false, "rules": "closed" },
              "type": [{ "code": "Quantity" }] },
            { "id": "Extension.value[x].value", "path": "Extension.value[x].value", "min": 1 }
        ] }
    });
    assert_eq!(check_snapshot(&extension), Vec::new());

    let mut loosened = extension.clone();
    loosened["snapshot"]["element"][2]["min"] = json!(0);
    assert_eq!(
        check_snapshot(&loosened),
        vec![differs("Extension.valueQuantity.value", "min")]
    );
}
