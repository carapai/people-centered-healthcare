//! Price Books and Price Rules as FHIR ChargeItemDefinitions (pricing-and-insurance.md § Records;
//! #120), written through the unit of work when a version is approved or retired, so
//! `/ChargeItemDefinition` reads like any other resource. Drafts are not projected.
//!
//! - **A book entry** is one ChargeItemDefinition: `code` the item, `version` the book's version,
//!   `applicability.effectivePeriod` the book's (R5), a `base` price component, and a property group per quantity
//!   tier, applicable when `%quantity >= min`.
//! - **A rule** is one ChargeItemDefinition whose property group has its FHIRPath applicability
//!   and one price component: a surcharge, a discount (stage 3, bundles included) or a deduction
//!   (stage 5, from the patient share), as a `factor` for a percentage or an `amount`.
//! - **Platform extensions** carry what ChargeItemDefinition cannot: the book's scope, and the
//!   rule's stage, priority, items and bundle.
//!
//! Each id is the UUIDv5 of the book and item, or of the rule, with its version, so every
//! version keeps its own resource and history is never overwritten.

use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, ResourceType, Transaction, WriteRequest};
use health_pricing::{Adjustment, Approval, Entry, PriceBook, PriceRule, Scope, Stage};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::PricingAppError;

/// The platform's canonical base.
const BASE: &str = "https://carapai.github.io/people-centered-healthcare/fhir";

/// The namespace of ChargeItemDefinition ids.
const NAMESPACE: Uuid = Uuid::from_u128(0x9b1e_44c7_2f6a_4d0e_b3a8_5c71_e2d9_0f46);

fn extension(name: &str) -> String {
    format!("{BASE}/StructureDefinition/{name}")
}

fn scope_code(scope: &Scope) -> String {
    match scope {
        Scope::PayerContract(id) => format!("payer-contract|{id}"),
        Scope::Program(id) => format!("program|{id}"),
        Scope::Department(id) => format!("department|{id}"),
        Scope::Facility(id) => format!("facility|{id}"),
        Scope::Organization(id) => format!("organization|{id}"),
        Scope::Tenant => "tenant".to_owned(),
    }
}

fn period(from: health_time::Timestamp, to: Option<health_time::Timestamp>) -> Value {
    let mut period = json!({ "start": from.to_string() });
    if let Some(to) = to {
        period["end"] = json!(to.to_string());
    }
    period
}

fn status(approval: &Approval) -> &'static str {
    match approval {
        Approval::Draft => "draft",
        Approval::Approved { .. } => "active",
    }
}

/// A decimal as an exact JSON number (serde_json's `arbitrary_precision`), never a string.
fn number(value: rust_decimal::Decimal) -> Value {
    value
        .normalize()
        .to_string()
        .parse::<serde_json::Number>()
        .map_or(Value::Null, Value::Number)
}

fn money(amount: rust_decimal::Decimal, currency: &str) -> Value {
    json!({ "value": number(amount), "currency": currency })
}

/// The id of the ChargeItemDefinition for `item` in `book`.
pub fn entry_id(book: &PriceBook, item: &str) -> String {
    Uuid::new_v5(
        &NAMESPACE,
        format!("book|{}|{}|{item}", book.id, book.version).as_bytes(),
    )
    .to_string()
}

/// The id of the ChargeItemDefinition for `rule`.
pub fn rule_id(rule: &PriceRule) -> String {
    Uuid::new_v5(
        &NAMESPACE,
        format!("rule|{}|{}", rule.id, rule.version).as_bytes(),
    )
    .to_string()
}

/// The ChargeItemDefinition of `entry` in `book`.
pub fn entry(book: &PriceBook, entry: &Entry) -> Value {
    let mut groups = vec![json!({
        "priceComponent": [{ "type": "base", "amount": money(entry.unit, &book.currency) }]
    })];
    for tier in &entry.tiers {
        groups.push(json!({
            "applicability": [{ "condition": {
                "language": "text/fhirpath",
                "expression": format!("%quantity >= {}", tier.min)
            } }],
            "priceComponent": [{ "type": "base", "amount": money(tier.unit, &book.currency) }]
        }));
    }
    json!({
        "resourceType": "ChargeItemDefinition",
        "id": entry_id(book, &entry.item),
        "extension": [{ "url": extension("price-book-scope"), "valueString": scope_code(&book.scope) }],
        "url": format!("{BASE}/ChargeItemDefinition/{}-{}", book.id, entry.item),
        "identifier": [{ "system": format!("{BASE}/sid/price-book"), "value": book.id }],
        "version": book.version.to_string(),
        "title": format!("{} in {}", entry.item, book.id),
        "status": status(&book.approval),
        "code": { "text": entry.item },
        "applicability": [{ "effectivePeriod": period(book.effective.from, book.effective.to) }],
        "propertyGroup": groups
    })
}

/// The ChargeItemDefinition of `rule`, priced in `currency`.
pub fn rule(rule: &PriceRule, currency: &str) -> Value {
    let (stage, kind) = match rule.stage {
        Stage::Surcharge => ("surcharge", "surcharge"),
        Stage::Discount => ("discount", "discount"),
        Stage::Subsidy => ("subsidy", "deduction"),
    };
    let mut component = Map::new();
    component.insert("type".into(), json!(kind));
    let mut extensions = vec![
        json!({ "url": extension("price-rule-stage"), "valueCode": stage }),
        json!({ "url": extension("price-rule-priority"), "valueInteger": rule.priority }),
    ];
    let items = match &rule.adjustment {
        Adjustment::Percent { percent } => {
            component.insert(
                "factor".into(),
                number(*percent / rust_decimal::Decimal::ONE_HUNDRED),
            );
            rule.items.clone()
        }
        Adjustment::Amount { amount } => {
            component.insert("amount".into(), money(*amount, currency));
            rule.items.clone()
        }
        Adjustment::Bundle { items, price, item } => {
            component.insert("amount".into(), money(*price, currency));
            extensions.push(json!({ "url": extension("price-rule-bundle"), "valueString": item }));
            items.clone()
        }
    };
    for item in items {
        extensions.push(json!({ "url": extension("price-rule-item"), "valueString": item }));
    }
    let mut group = Map::new();
    if let Some(expression) = &rule.applicability {
        group.insert(
            "applicability".into(),
            json!([{ "condition": { "language": "text/fhirpath", "expression": expression } }]),
        );
    }
    group.insert("priceComponent".into(), json!([Value::Object(component)]));
    json!({
        "resourceType": "ChargeItemDefinition",
        "id": rule_id(rule),
        "extension": extensions,
        "url": format!("{BASE}/ChargeItemDefinition/rule-{}", rule.id),
        "identifier": [{ "system": format!("{BASE}/sid/price-rule"), "value": rule.id }],
        "version": rule.version.to_string(),
        "title": rule.id,
        "status": status(&rule.approval),
        "applicability": [{ "effectivePeriod": period(rule.effective.from, rule.effective.to) }],
        "propertyGroup": [Value::Object(group)]
    })
}

fn store_error(error: impl ToString) -> PricingAppError {
    PricingAppError::Store(error.to_string())
}

/// Writes `resource` as a ChargeItemDefinition, unless its head already says the same.
pub(crate) async fn put<T: Transaction>(
    tx: &mut T,
    resource: Value,
) -> Result<(), PricingAppError> {
    let kind: ResourceType = "ChargeItemDefinition".parse().map_err(store_error)?;
    let id_text = resource["id"].as_str().unwrap_or_default().to_owned();
    let id = ResourceId::from_uuid(id_text.parse().map_err(store_error)?).map_err(store_error)?;
    let head = tx.read(&kind, id).await?;
    let precondition = match &head {
        Some(head) => {
            let mut stored: Value =
                serde_json::from_str(head.json.as_str()).map_err(store_error)?;
            if let Some(object) = stored.as_object_mut() {
                object.remove("meta");
            }
            if !head.version.deleted && stored == resource {
                return Ok(());
            }
            Precondition::IfMatch(head.version.version_id)
        }
        None => Precondition::MustNotExist,
    };
    tx.write(WriteRequest {
        resource_type: kind,
        id,
        json: RawJson::from_validated(resource.to_string()),
        op_id: OpId::new(),
        precondition,
    })
    .await?;
    Ok(())
}
