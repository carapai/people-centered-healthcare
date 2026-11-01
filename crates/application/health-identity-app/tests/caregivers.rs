//! Caregiver grants over SurrealDB (memory engine): staff record a verified grant, revoke it,
//! and record an adolescent's consent; each change writes the grant, its RelatedPerson and its
//! Consent together, audited.

use std::sync::{Arc, Mutex};

use health_consent::{Authority, Confidentiality, Day, Refused, Revoker, Scope};
use health_identity_app::caregivers::{
    CaregiverAudit, CaregiverError, Caregivers, NewGrant, grant_id,
};
use health_persistence::{RawJson, ResourceType, Store, Transaction, WriteError};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::{SystemClock, Timestamp};
use serde_json::Value;

#[derive(Clone, Default)]
struct Audit(Arc<Mutex<Vec<String>>>);

impl CaregiverAudit<SurrealTx> for Audit {
    async fn written(
        &self,
        _tx: &mut SurrealTx,
        actor: &str,
        reason: &str,
        json: &RawJson,
    ) -> Result<(), WriteError> {
        let v: Value = serde_json::from_str(json.as_str()).unwrap_or_default();
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "{actor} {reason} {}",
                v["resourceType"].as_str().unwrap_or_default()
            ));
        Ok(())
    }
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "consent".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

const PATIENT: &str = "018f0000-0000-7000-8000-0000000000aa";

fn mother() -> NewGrant {
    NewGrant {
        caregiver: "account-mother".into(),
        phone: "+256772100009".into(),
        patient: PATIENT.into(),
        relationship: "MTH".into(),
        authority: Authority::Guardian,
        scopes: vec![Scope::ViewRecords, Scope::BookAppointments],
        expires: Timestamp::from_second(1_900_000_000).expect("time"),
        document: "birth-certificate".into(),
    }
}

async fn read(store: &SurrealStore, kind: &str, id: &str) -> Value {
    let mut tx = store.begin().await.expect("tx");
    let stored = tx
        .read(
            &kind.parse::<ResourceType>().expect("type"),
            id.parse().expect("id"),
        )
        .await
        .expect("read")
        .expect("stored");
    serde_json::from_str(stored.json.as_str()).expect("json")
}

fn today() -> Day {
    Day::new(2027, 1, 15).expect("day")
}

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time")
}

#[tokio::test]
async fn staff_record_a_verified_grant_with_its_related_person_and_consent() {
    let store = store().await;
    let audit = Audit::default();
    let caregivers = Caregivers::new(store.clone(), audit.clone(), Confidentiality::default());
    let grant = caregivers
        .record("clerk-1", mother())
        .await
        .expect("recorded");
    assert_eq!(grant.id, grant_id("account-mother", PATIENT));
    assert!(grant.verification.is_some());
    assert_eq!(
        caregivers.record("clerk-1", mother()).await,
        Err(CaregiverError::AlreadyGranted)
    );

    let consent = read(&store, "Consent", &grant.id).await;
    assert_eq!(consent["status"], "active");
    assert_eq!(
        consent["subject"]["reference"],
        format!("Patient/{PATIENT}")
    );
    assert_eq!(
        consent["provision"][0]["action"][0]["coding"][0]["code"],
        "view-records"
    );
    assert_eq!(
        consent["verification"][0]["verificationType"]["text"],
        "birth-certificate"
    );
    let related = read(&store, "RelatedPerson", &grant.related_person).await;
    assert_eq!(related["relationship"][0]["coding"][0]["code"], "MTH");
    assert_eq!(
        audit.0.lock().expect("lock").as_slice(),
        [
            "clerk-1 caregiver-record RelatedPerson",
            "clerk-1 caregiver-record Consent"
        ]
    );

    assert_eq!(
        caregivers.of_patient(PATIENT).await.expect("list"),
        vec![grant.clone()]
    );
    assert_eq!(
        caregivers
            .of_caregiver("account-mother")
            .await
            .expect("list"),
        vec![grant]
    );
    assert!(
        caregivers
            .of_caregiver("someone-else")
            .await
            .expect("list")
            .is_empty()
    );
}

#[tokio::test]
async fn a_grant_needs_a_scope_and_the_document_checked() {
    let caregivers = Caregivers::new(store().await, Audit::default(), Confidentiality::default());
    let mut none = mother();
    none.scopes.clear();
    assert!(matches!(
        caregivers.record("clerk-1", none).await,
        Err(CaregiverError::Invalid(_))
    ));
    let mut undocumented = mother();
    undocumented.document = " ".into();
    assert!(matches!(
        caregivers.record("clerk-1", undocumented).await,
        Err(CaregiverError::Invalid(_))
    ));
}

#[tokio::test]
async fn revoking_makes_the_consent_inactive_and_a_new_grant_may_follow() {
    let store = store().await;
    let caregivers = Caregivers::new(store.clone(), Audit::default(), Confidentiality::default());
    let grant = caregivers
        .record("clerk-1", mother())
        .await
        .expect("recorded");
    // A minor may not end their guardian's access.
    assert!(matches!(
        caregivers
            .revoke(
                &grant.id,
                Revoker::Patient,
                "patient",
                Some("2012-03-10"),
                today(),
                now()
            )
            .await,
        Err(CaregiverError::Refused(Refused::MayNotRevoke(_)))
    ));
    let revoked = caregivers
        .revoke(
            &grant.id,
            Revoker::Staff {
                staff: "clerk-2".into(),
            },
            "clerk-2",
            None,
            today(),
            now(),
        )
        .await
        .expect("revoked");
    assert!(revoked.revoked.is_some());
    assert_eq!(
        read(&store, "Consent", &grant.id).await["status"],
        "inactive"
    );
    let again = caregivers
        .record("clerk-1", mother())
        .await
        .expect("recorded again");
    assert!(again.revoked.is_none());
    assert_eq!(read(&store, "Consent", &grant.id).await["status"], "active");
    assert_eq!(
        caregivers
            .revoke("no-such", Revoker::Patient, "p", None, today(), now())
            .await,
        Err(CaregiverError::NotFound)
    );
}

#[tokio::test]
async fn an_adolescents_consent_is_recorded_on_a_guardians_grant_only() {
    let store = store().await;
    let caregivers = Caregivers::new(store.clone(), Audit::default(), Confidentiality::default());
    let grant = caregivers
        .record("clerk-1", mother())
        .await
        .expect("recorded");
    let consented = caregivers
        .adolescent_consent(&grant.id, true, "clerk-1")
        .await
        .expect("consent");
    assert!(consented.adolescent_consents);
    let consent = read(&store, "Consent", &grant.id).await;
    assert!(
        consent["extension"]
            .as_array()
            .expect("extensions")
            .iter()
            .any(|e| e["valueBoolean"] == true)
    );
    let mut delegate = mother();
    delegate.caregiver = "account-son".into();
    delegate.authority = Authority::Delegated;
    let delegated = caregivers
        .record("clerk-1", delegate)
        .await
        .expect("recorded");
    assert!(matches!(
        caregivers
            .adolescent_consent(&delegated.id, true, "clerk-1")
            .await,
        Err(CaregiverError::Invalid(_))
    ));
}
