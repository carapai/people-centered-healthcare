//! The facility registry's places over SurrealDB (memory engine; ADR 0094, #869): an NHFR import
//! previewed and applied creates the hierarchy as Organizations, a district's area lists its
//! facilities, and a re-import with a closed facility marks it inactive and keeps it.

use std::sync::{Arc, Mutex};

use health_facility_registry::{
    AdministrativeAreas, Area, District, NATIONAL, PlaceLevel, parse_nhfr_csv, place_id,
    plan_import,
};
use health_fhir_search::Registry;
use health_fhir_validation::validate_structure;
use health_persistence::{RawJson, Store, WriteError};
use health_registries_app::AuditTrail;
use health_registries_app::places::{
    apply_import, level_of, location_json, organization_json, places,
};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::SystemClock;

#[derive(Default, Clone)]
struct Recorded(Arc<Mutex<Vec<String>>>);

impl AuditTrail<SurrealTx> for Recorded {
    async fn written(
        &self,
        _tx: &mut SurrealTx,
        actor: &str,
        reason: &str,
        _json: &RawJson,
    ) -> Result<(), WriteError> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{actor} {reason}"));
        Ok(())
    }

    async fn recorded(
        &self,
        _tx: &mut SurrealTx,
        _actor: &str,
        _reason: &str,
        _what: &str,
    ) -> Result<(), WriteError> {
        Ok(())
    }
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "registry".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
}

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

const FIRST: &str = "nhfr_id,name,district,level,ownership,status,latitude,longitude\n\
    KAM013,Kisenyi HC IV,district-kampala,HC IV,Government,open,0.3136,32.5664\n\
    KAM020,Kawempe HC III,district-kampala,HC III,Government,open,,\n\
    WAK001,Wakiso HC IV,district-wakiso,HC IV,PNFP,open,0.4044,32.4594\n";

#[tokio::test]
async fn an_nhfr_import_creates_the_hierarchy_and_closes_facilities_on_reimport() {
    let store = store().await;
    let audit = Recorded::default();
    let rows = parse_nhfr_csv(FIRST).expect("rows");
    let current = places(&mut store.begin().await.expect("tx"))
        .await
        .expect("places");
    assert!(current.is_empty());
    let plan = plan_import(&current, &areas(), &rows).expect("preview");
    assert_eq!((plan.hierarchy.len(), plan.new.len()), (4, 3));
    let written = apply_import(&store, &audit, "admin", &plan)
        .await
        .expect("apply");
    assert_eq!(written, 7);
    assert_eq!(
        audit.0.lock().expect("audit").len(),
        9,
        "7 places, 2 locations"
    );

    let registry = places(&mut store.begin().await.expect("tx"))
        .await
        .expect("places");
    assert_eq!(registry.len(), 7);
    assert_eq!(registry.level_of("district-kampala"), PlaceLevel::District);
    assert_eq!(
        registry.facilities_in("district-kampala"),
        ["KAM013", "KAM020"]
    );
    assert_eq!(registry.facilities_in(NATIONAL).len(), 3);
    let kisenyi = registry.get("KAM013").expect("KAM013");
    assert_eq!(kisenyi.care_level.as_deref(), Some("HC IV"));
    assert_eq!(
        kisenyi.coordinates.as_ref().map(|c| c.longitude.as_str()),
        Some("32.5664")
    );
    // Read back, the registry equals what was imported: the same file again changes nothing.
    let again = plan_import(&registry, &areas(), &rows).expect("preview");
    assert!(again.writes().is_empty(), "{again:?}");
    assert_eq!(again.unchanged, 3);

    let closed = parse_nhfr_csv(
        "nhfr_id,name,district,level,ownership,status,latitude,longitude\n\
         KAM013,Kisenyi HC IV,district-kampala,HC IV,Government,open,0.3136,32.5664\n\
         KAM020,Kawempe HC III,district-kampala,HC III,Government,closed,,\n\
         WAK001,Wakiso HC IV,district-wakiso,HC IV,PNFP,open,0.4044,32.4594\n",
    )
    .expect("rows");
    let plan = plan_import(&registry, &areas(), &closed).expect("preview");
    assert_eq!(plan.closed.len(), 1);
    apply_import(&store, &audit, "admin", &plan)
        .await
        .expect("apply");
    let registry = places(&mut store.begin().await.expect("tx"))
        .await
        .expect("places");
    assert_eq!(registry.get("KAM020").map(|p| p.active), Some(false));
    assert_eq!(
        registry.facilities_in("district-kampala"),
        ["KAM013", "KAM020"],
        "a closed facility is kept"
    );
}

#[test]
fn places_are_valid_organizations_and_locations() {
    let rows = parse_nhfr_csv(FIRST).expect("rows");
    let plan = plan_import(&Default::default(), &areas(), &rows).expect("plan");
    for place in plan.writes() {
        let organization = organization_json(place);
        assert_eq!(
            organization["id"],
            place_id(&place.code).to_string(),
            "{organization}"
        );
        assert_eq!(level_of(&organization), Some(place.level));
        let problems = validate_structure(&organization.to_string());
        assert!(problems.is_empty(), "{problems:?}");
        if let Some(location) = location_json(place) {
            let problems = validate_structure(&location.to_string());
            assert!(problems.is_empty(), "{problems:?}");
        }
    }
}
