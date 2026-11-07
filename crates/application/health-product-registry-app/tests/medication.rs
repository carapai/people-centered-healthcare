//! The product master as FHIR R5 Medication (#128 follow-up): a product written as a Medication
//! is valid R5 and reads back the same; a Medication that is not a stock product reads as none.

use health_fhir_validation::validate_structure;
use health_product_registry::{Coding, Medicine, Product, resolve};
use health_product_registry_app::{medication, product, products};
use serde_json::json;

fn al() -> Product {
    Product {
        code: "al-20-120".into(),
        name: "Artemether/lumefantrine 20/120 mg".into(),
        gtin: Some("09501101530003".into()),
        base_unit: "tablet".into(),
        pack_sizes: vec![24, 6],
        medicines: vec![Coding {
            system: "http://www.whocc.no/atc".into(),
            code: "P01BF01".into(),
        }],
        synonyms: vec!["Coartem".into()],
        active: true,
    }
}

#[test]
fn a_product_is_a_valid_medication_and_reads_back_the_same() {
    let resource = medication(&al(), "0190f1a2-7c3d-7e4f-8a5b-00000000a001");
    assert_eq!(
        validate_structure(&resource.to_string()),
        vec![],
        "{resource}"
    );
    assert_eq!(product(&resource), Some(al()));
    let mut retired = al();
    retired.active = false;
    retired.gtin = None;
    retired.medicines = vec![];
    retired.synonyms = vec![];
    let resource = medication(&retired, "0190f1a2-7c3d-7e4f-8a5b-00000000a002");
    assert_eq!(
        validate_structure(&resource.to_string()),
        vec![],
        "{resource}"
    );
    assert_eq!(product(&resource), Some(retired));
}

#[test]
fn only_stock_products_are_read_and_they_resolve_prescriptions() {
    let plain =
        json!({ "resourceType": "Medication", "id": "m1", "code": { "text": "Something" } });
    let wrong = json!({ "resourceType": "Medication", "status": "entered-in-error" });
    let found = [plain, wrong, medication(&al(), "m2")];
    let catalogue = products(&found);
    assert_eq!(catalogue, vec![al()]);
    let written = Medicine {
        text: "Coartem".into(),
        coding: None,
    };
    assert_eq!(resolve(&catalogue, &written).product(), Some("al-20-120"));
}
