//! Architecture test: every table has exactly one Owner, and the registry, the migrated schema
//! and the ownership matrix agree.

use std::collections::{BTreeMap, BTreeSet};

use health_surrealdb::ownership::{
    self, INFRASTRUCTURE_TABLES, MODULE_TABLES, UNIT_OF_WORK, owner_of,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, Target, migrate};

/// Every table to its Owner. History tables belong to their head table's Owner.
fn declared() -> BTreeMap<String, Vec<&'static str>> {
    let mut owners: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    for entry in ownership::resources() {
        owners
            .entry(entry.table.to_owned())
            .or_default()
            .push(entry.owner);
        owners
            .entry(format!("{}_history", entry.table))
            .or_default()
            .push(entry.owner);
    }
    for table in INFRASTRUCTURE_TABLES {
        owners
            .entry((*table).to_owned())
            .or_default()
            .push(UNIT_OF_WORK);
    }
    for (table, owner) in MODULE_TABLES {
        owners.entry((*table).to_owned()).or_default().push(owner);
    }
    owners
}

#[test]
fn every_table_has_exactly_one_owner() {
    for (table, owners) in declared() {
        assert_eq!(owners.len(), 1, "{table} has owners {owners:?}");
    }
}

#[test]
fn partitions_are_disjoint_and_move_records_to_another_owner() {
    for entry in ownership::resources() {
        let keys: BTreeSet<(&str, &str)> = entry
            .partitions
            .iter()
            .map(|p| (p.system, p.code))
            .collect();
        assert_eq!(
            keys.len(),
            entry.partitions.len(),
            "{} has overlapping partitions",
            entry.resource_type
        );
        for p in entry.partitions {
            assert_ne!(
                p.owner, entry.owner,
                "{} partition {} repeats the default owner",
                entry.resource_type, p.code
            );
        }
    }
}

#[tokio::test]
async fn the_migrated_schema_has_exactly_the_declared_tables() {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let mut info = connection
        .query("RETURN object::keys((INFO FOR DB).tables)")
        .await
        .expect("info");
    let tables: BTreeSet<String> = info
        .take::<Vec<String>>(0)
        .expect("tables")
        .into_iter()
        .collect();
    let declared: BTreeSet<String> = declared().into_keys().collect();
    assert_eq!(tables, declared);
}

#[test]
fn category_decides_the_owner_of_split_types() {
    let lab = serde_json::json!({"resourceType":"Observation","category":[{"coding":[{"system":"http://terminology.hl7.org/CodeSystem/observation-category","code":"laboratory"}]}]});
    let vitals = serde_json::json!({"resourceType":"Observation","category":[{"coding":[{"system":"http://terminology.hl7.org/CodeSystem/observation-category","code":"vital-signs"}]}]});
    let imaging = serde_json::json!({"resourceType":"DiagnosticReport","category":[{"coding":[{"system":"http://terminology.hl7.org/CodeSystem/v2-0074","code":"RAD"}]}]});
    let referral = serde_json::json!({"resourceType":"ServiceRequest","category":[{"coding":[{"system":"http://snomed.info/sct","code":"3457005"}]}]});
    assert_eq!(owner_of("Observation", &lab), Some("laboratory"));
    assert_eq!(owner_of("Observation", &vitals), Some("clinical-data"));
    assert_eq!(
        owner_of("Observation", &serde_json::json!({})),
        Some("clinical-data")
    );
    assert_eq!(owner_of("DiagnosticReport", &imaging), Some("imaging"));
    assert_eq!(owner_of("ServiceRequest", &referral), Some("referrals"));
    assert_eq!(
        owner_of("Patient", &serde_json::json!({})),
        Some("client-registry")
    );
    assert_eq!(owner_of("Basic", &serde_json::json!({})), None);
}

#[test]
fn every_owner_is_a_module_in_the_ownership_doc() {
    let doc = include_str!("../../../../docs/architecture/resource-ownership.md");
    let modules_section = &doc[doc
        .find("## Bounded contexts and modules")
        .expect("section")
        ..doc.find("## Context map").expect("section")];
    let owners: BTreeSet<&str> = ownership::resources()
        .iter()
        .flat_map(|r| std::iter::once(r.owner).chain(r.partitions.iter().map(|p| p.owner)))
        // A Provenance is its target's Owner's, not a module of its own (rule 7).
        .filter(|owner| *owner != ownership::TARGET_OWNER)
        .collect();
    for owner in owners {
        assert!(
            modules_section.contains(owner),
            "{owner} is not a module in resource-ownership.md"
        );
    }
}
