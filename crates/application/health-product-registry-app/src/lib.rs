//! The product master as FHIR R5 Medication resources (resource-ownership.md: Medication is
//! product-registry's; inventory-and-procurement.md § Records). Central is their authority.
//!
//! A [`Product`] is a Medication:
//! - `identifier`: the product's code ([`PRODUCT_SYSTEM`]) and its GTIN ([`GTIN_SYSTEM`]);
//! - `code`: the medicine codings it fills, and its name as `text`;
//! - `status`: `active` or `inactive`;
//! - extensions: the base unit ([`BASE_UNIT`]), each pack size ([`PACK_SIZE`]) and each synonym
//!   ([`SYNONYM`]).
//!
//! [`medication`] writes one, [`product`] reads one back (`None` for a Medication that is not a
//! stock product), and [`products`] reads them all from what a search found. [`Products::save`]
//! keeps one, its code unique and unchanging.

use health_product_registry::{Coding, Product};
use serde_json::{Value, json};

/// The identifier system of a product's code (the Stock Unit's `product`).
pub const PRODUCT_SYSTEM: &str = "urn:health:product";
/// The identifier system of a GS1 GTIN.
pub const GTIN_SYSTEM: &str = "https://www.gs1.org/gtin";
/// The base-unit extension.
pub const BASE_UNIT: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/product-base-unit";
/// The pack-size extension, one per size, in base units.
pub const PACK_SIZE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/product-pack-size";
/// The synonym extension, one per name.
pub const SYNONYM: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/product-synonym";

/// `product` as a Medication with `id`.
#[must_use]
pub fn medication(product: &Product, id: &str) -> Value {
    let mut identifier = vec![json!({ "system": PRODUCT_SYSTEM, "value": product.code })];
    if let Some(gtin) = &product.gtin {
        identifier.push(json!({ "system": GTIN_SYSTEM, "value": gtin }));
    }
    let mut extension = vec![json!({ "url": BASE_UNIT, "valueString": product.base_unit })];
    extension.extend(
        product
            .pack_sizes
            .iter()
            .map(|size| json!({ "url": PACK_SIZE, "valueUnsignedInt": size })),
    );
    extension.extend(
        product
            .synonyms
            .iter()
            .map(|name| json!({ "url": SYNONYM, "valueString": name })),
    );
    let mut code = json!({ "text": product.name });
    if !product.medicines.is_empty() {
        code["coding"] = product
            .medicines
            .iter()
            .map(|c| json!({ "system": c.system, "code": c.code }))
            .collect();
    }
    json!({
        "resourceType": "Medication",
        "id": id,
        "extension": extension,
        "identifier": identifier,
        "code": code,
        "status": if product.active { "active" } else { "inactive" },
    })
}

fn identified<'a>(resource: &'a Value, system: &str) -> Option<&'a str> {
    resource["identifier"]
        .as_array()?
        .iter()
        .find(|i| i["system"] == system)?["value"]
        .as_str()
}

fn extensions<'a>(resource: &'a Value, url: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    resource["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(move |e| e["url"] == url)
}

/// The stock product a Medication is, if it is one: it names a product code, a name and a base
/// unit.
#[must_use]
pub fn product(resource: &Value) -> Option<Product> {
    if resource["resourceType"] != "Medication" || resource["status"] == "entered-in-error" {
        return None;
    }
    Some(Product {
        code: identified(resource, PRODUCT_SYSTEM)?.to_owned(),
        name: resource["code"]["text"].as_str()?.to_owned(),
        gtin: identified(resource, GTIN_SYSTEM).map(str::to_owned),
        base_unit: extensions(resource, BASE_UNIT)
            .find_map(|e| e["valueString"].as_str())?
            .to_owned(),
        pack_sizes: extensions(resource, PACK_SIZE)
            .filter_map(|e| u32::try_from(e["valueUnsignedInt"].as_u64()?).ok())
            .collect(),
        medicines: resource["code"]["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                Some(Coding {
                    system: c["system"].as_str()?.to_owned(),
                    code: c["code"].as_str()?.to_owned(),
                })
            })
            .collect(),
        synonyms: extensions(resource, SYNONYM)
            .filter_map(|e| e["valueString"].as_str().map(str::to_owned))
            .collect(),
        active: resource["status"] != "inactive",
    })
}

/// The stock products among `found`.
pub fn products<'a>(found: impl IntoIterator<Item = &'a Value>) -> Vec<Product> {
    found.into_iter().filter_map(product).collect()
}

mod products;

pub use products::{ProductError, Products};
