//! Tiers 3–4 on the write path (#91): with the platform core package's profiles active and its
//! terminology served, a write is validated against the profiles it claims and those its type
//! requires, bindings included. Errors refuse it with an OperationOutcome; without active
//! profiles nothing changes.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use health_fhir_search::Unrestricted;
use health_fhir_server::{ActiveProfiles, ActiveTerminology, FhirServer, Profiles};
use health_fhir_validation::ProfileSet;
use health_packages::Files;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_terminology::Terminology;
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const BASE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition";

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

fn core_package() -> Files {
    std::fs::read_dir(root().join("../../../packages/fhir-core-package/package"))
        .expect("the core package")
        .map(|entry| {
            let path = entry.expect("entry").path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("name")
                .to_owned();
            (name, std::fs::read(&path).expect("read"))
        })
        .collect()
}

/// The core package's profiles compiled, with HL7's SimpleQuantity from its dependency, as
/// activation will do; `required` names what each type must meet.
fn profiles(required: &[(&str, &str)]) -> Profiles {
    let mut definitions: Vec<Value> = core_package()
        .iter()
        .filter(|(name, _)| name.starts_with("StructureDefinition-"))
        .map(|(_, bytes)| serde_json::from_slice(bytes).expect("json"))
        .collect();
    definitions.push(read(&root().join(
        "../../contracts/health-fhir-validation/tests/fixtures/profiles/StructureDefinition-SimpleQuantity.json",
    )));
    let mut by_type: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (kind, profile) in required {
        by_type
            .entry((*kind).to_owned())
            .or_default()
            .push(format!("{BASE}/{profile}"));
    }
    Profiles {
        set: Arc::new(ProfileSet::compile(&definitions).expect("compile")),
        required: by_type,
    }
}

fn terminology() -> Terminology {
    let mut terminology = Terminology::new();
    health_packages::terminology::load(&mut terminology, &core_package()).expect("load");
    terminology
}

async fn app(profiles: Option<Profiles>) -> Router {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "validation-http".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let server = FhirServer::new(store, "http://test/fhir", Arc::new(Unrestricted))
        .with_terminology(ActiveTerminology::new(terminology()));
    match profiles {
        Some(profiles) => server.with_profiles(ActiveProfiles::new(profiles)),
        None => server,
    }
    .router()
}

async fn create(app: &Router, resource: &Value) -> (StatusCode, Value) {
    let kind = resource["resourceType"].as_str().expect("type");
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/{kind}"))
                .header("content-type", "application/fhir+json")
                .body(Body::from(resource.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn example(name: &str) -> Value {
    read(
        &root()
            .join("../../../packages/fhir-core-package/examples")
            .join(name),
    )
}

fn claiming(mut resource: Value, profile: &str) -> Value {
    resource["meta"] = json!({ "profile": [format!("{BASE}/{profile}")] });
    resource
}

#[tokio::test]
async fn a_claimed_profile_is_enforced_with_its_bindings() {
    let app = app(Some(profiles(&[]))).await;
    let mapping = claiming(
        example("Library-anc-visit-mapping.json"),
        "ExtractionMappingLibrary",
    );
    let (status, body) = create(&app, &mapping).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let mut unknown_mode = mapping;
    unknown_mode["contained"][0]["extension"][0]["extension"][0]["valueCode"] = json!("sometimes");
    let (status, outcome) = create(&app, &unknown_mode).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{outcome}");
    let issues = outcome["issue"].as_array().expect("issues");
    assert!(
        issues
            .iter()
            .any(|i| i["code"] == "code-invalid" && i["severity"] == "error"),
        "{outcome}"
    );

    let mut no_content = claiming(
        example("Library-patient-matching-rules.json"),
        "MatchingRulesLibrary",
    );
    no_content
        .as_object_mut()
        .expect("object")
        .remove("content");
    let (status, outcome) = create(&app, &no_content).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{outcome}");
}

#[tokio::test]
async fn a_type_s_required_profile_applies_without_a_claim() {
    let app = app(Some(profiles(&[(
        "PlanDefinition",
        "ProgramPlanDefinition",
    )])))
    .await;
    let program = example("PlanDefinition-art-program.json");
    let (status, body) = create(&app, &program).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let mut order_set = program;
    order_set["type"]["coding"][0]["code"] = json!("order-set");
    let (status, outcome) = create(&app, &order_set).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{outcome}");
}

#[tokio::test]
async fn a_profile_the_node_does_not_have_is_refused() {
    let app = app(Some(profiles(&[]))).await;
    let claimed = claiming(example("Library-opd-visit-workflow.json"), "NoSuchProfile");
    let (status, outcome) = create(&app, &claimed).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{outcome}");
    assert_eq!(outcome["issue"][0]["code"], "processing");
}

#[tokio::test]
async fn without_active_profiles_tiers_3_and_4_do_not_run() {
    let app = app(None).await;
    let mut no_content = claiming(
        example("Library-patient-matching-rules.json"),
        "MatchingRulesLibrary",
    );
    no_content
        .as_object_mut()
        .expect("object")
        .remove("content");
    let (status, body) = create(&app, &no_content).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}
