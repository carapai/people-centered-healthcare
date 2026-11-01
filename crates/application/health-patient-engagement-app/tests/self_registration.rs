//! Self-registration over SurrealDB (memory engine), with a fake client registry: a phone
//! account is never linked to an existing Patient automatically, and below level3 the account
//! sees only what it submitted.

use std::sync::{Arc, Mutex};

use health_client_registry::person::PortalAccess;
use health_identifiers::ResourceId;
use health_patient_engagement::registration::Status;
use health_patient_engagement_app::{
    AuditTrail, Candidate, EngagementError, PatientRegistry, QUESTIONNAIRE, SelfRegistration,
};
use health_persistence::{RawJson, ResourceType, Store, Transaction, WriteError};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, SurrealTx, Target, migrate,
};
use health_time::SystemClock;
use serde_json::{Value, json};

const PHONE: &str = "+256772123456";

#[derive(Clone, Default)]
struct Registry {
    candidates: Option<Vec<Candidate>>,
    created: Arc<Mutex<Vec<(ResourceId, Value)>>>,
}

impl PatientRegistry for Registry {
    async fn find(&self, _patient: &Value) -> Result<Option<Vec<Candidate>>, EngagementError> {
        Ok(self.candidates.clone())
    }

    async fn create_self_registered(
        &self,
        id: ResourceId,
        patient: &Value,
        _actor: &str,
    ) -> Result<(), EngagementError> {
        self.created
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((id, patient.clone()));
        Ok(())
    }
}

#[derive(Clone, Default)]
struct Audit(Arc<Mutex<Vec<String>>>);

impl AuditTrail<SurrealTx> for Audit {
    async fn written(
        &self,
        _tx: &mut SurrealTx,
        actor: &str,
        _reason: &str,
        json: &RawJson,
    ) -> Result<(), WriteError> {
        let v: Value = serde_json::from_str(json.as_str()).unwrap_or_default();
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "{actor} {}",
                v["resourceType"].as_str().unwrap_or_default()
            ));
        Ok(())
    }
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "portal".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn response() -> Value {
    let text =
        |link: &str, value: &str| json!({ "linkId": link, "answer": [{ "valueString": value }] });
    json!({
        "resourceType": "QuestionnaireResponse",
        "questionnaire": QUESTIONNAIRE,
        "status": "completed",
        "item": [
            text("given", "Grace"),
            text("family", "Nakato"),
            { "linkId": "birthDate", "answer": [{ "valueDate": "1990-04-12" }] },
            { "linkId": "gender", "answer": [{ "valueCoding": { "code": "female" } }] },
        ],
    })
}

async fn read(store: &SurrealStore, kind: &str, id: ResourceId) -> Value {
    let mut tx = store.begin().await.expect("tx");
    let stored = tx
        .read(&kind.parse::<ResourceType>().expect("type"), id)
        .await
        .expect("read")
        .expect("stored");
    serde_json::from_str(stored.json.as_str()).expect("json")
}

type Use = SelfRegistration<SurrealStore, Registry, Audit>;

#[tokio::test]
async fn no_match_creates_a_self_registered_patient_that_sees_only_what_it_submitted() {
    let store = store().await;
    let registry = Registry {
        candidates: Some(Vec::new()),
        ..Registry::default()
    };
    let audit = Audit::default();
    let use_case = Use::new(store.clone(), registry.clone(), audit.clone());
    assert_eq!(use_case.status("acct-1").await.expect("status"), None);

    let view = use_case
        .submit("acct-1", PHONE, &response())
        .await
        .expect("submit");
    assert_eq!(view.status, Status::Completed);
    assert_eq!(view.access, PortalAccess::SelfSubmitted);
    let created = registry.created.lock().expect("lock").clone();
    let [(patient_id, patient)] = created.as_slice() else {
        panic!("one patient: {created:?}");
    };
    assert_eq!(view.patient, Some(patient_id.to_string()));
    assert_eq!(patient["name"][0]["family"], "Nakato");
    assert_eq!(patient["birthDate"], "1990-04-12");
    assert_eq!(patient["gender"], "female");
    assert_eq!(patient["telecom"][0]["value"], PHONE);

    let person = read(&store, "Person", Use::person_id("acct-1")).await;
    assert_eq!(
        person["link"][0]["target"]["reference"],
        format!("Patient/{patient_id}")
    );
    assert_eq!(person["link"][0]["assurance"], "level1");
    let task = read(&store, "Task", Use::request_id("acct-1")).await;
    assert_eq!(task["status"], "completed");
    assert_eq!(task["intent"], "proposal");
    assert_eq!(task["output"][0]["type"]["text"], "created");
    assert_eq!(use_case.status("acct-1").await.expect("status"), Some(view));
    assert_eq!(
        audit.0.lock().expect("lock").as_slice(),
        [
            "account acct-1 QuestionnaireResponse",
            "account acct-1 Person",
            "account acct-1 Task"
        ]
    );

    // One registration per account.
    assert_eq!(
        use_case.submit("acct-1", PHONE, &response()).await,
        Err(EngagementError::AlreadySubmitted)
    );
}

#[tokio::test]
async fn a_certain_match_on_a_phone_account_goes_to_review_and_links_nothing() {
    let store = store().await;
    let registry = Registry {
        candidates: Some(vec![Candidate {
            patient: "existing-1".into(),
            certain: true,
        }]),
        ..Registry::default()
    };
    let use_case = Use::new(store.clone(), registry.clone(), Audit::default());
    let view = use_case
        .submit("acct-2", PHONE, &response())
        .await
        .expect("submit");
    assert_eq!(view.status, Status::Review);
    assert_eq!(view.patient, None);
    assert_eq!(view.access, PortalAccess::None);
    assert!(registry.created.lock().expect("lock").is_empty());
    let person = read(&store, "Person", Use::person_id("acct-2")).await;
    assert_eq!(person["link"], json!([]));
    let task = read(&store, "Task", Use::request_id("acct-2")).await;
    assert_eq!(task["status"], "in-progress");
    assert!(
        task["input"]
            .as_array()
            .expect("inputs")
            .iter()
            .any(|i| i["type"]["text"] == "candidate"
                && i["valueReference"]["reference"] == "Patient/existing-1")
    );
    assert_eq!(use_case.status("acct-2").await.expect("status"), Some(view));
}

#[tokio::test]
async fn too_little_to_match_on_also_goes_to_review() {
    let store = store().await;
    let use_case = Use::new(store, Registry::default(), Audit::default());
    let view = use_case
        .submit("acct-3", PHONE, &response())
        .await
        .expect("submit");
    assert_eq!(view.status, Status::Review);
}

#[tokio::test]
async fn answers_are_checked() {
    let use_case = Use::new(store().await, Registry::default(), Audit::default());
    let mut wrong = response();
    wrong["questionnaire"] = json!("urn:other");
    assert!(matches!(
        use_case.submit("a", PHONE, &wrong).await,
        Err(EngagementError::Invalid(_))
    ));
    let mut missing = response();
    missing["item"].as_array_mut().expect("items").remove(1);
    assert_eq!(
        use_case.submit("a", PHONE, &missing).await,
        Err(EngagementError::Invalid(
            "the answer `family` is required".into()
        ))
    );
    let mut date = response();
    date["item"][2]["answer"][0]["valueDate"] = json!("12/04/1990");
    assert!(matches!(
        use_case.submit("a", PHONE, &date).await,
        Err(EngagementError::Invalid(_))
    ));
    assert_eq!(
        use_case.status("a").await.expect("status"),
        None,
        "nothing written"
    );
}
