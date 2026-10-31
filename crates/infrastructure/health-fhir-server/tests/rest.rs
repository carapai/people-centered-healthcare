//! R5 REST conformance fixtures (`tests/fixtures/rest/*.json`) and the check that the
//! CapabilityStatement matches behaviour, on SurrealDB 3.3.5 (in-memory engine).
//!
//! A fixture is a list of steps: a request, the expected status, headers and JSON (by JSON
//! pointer), and values to save for later steps. `{{name}}` substitutes a saved value, and
//! `{{uuid:name}}` a fresh UUIDv7 per fixture.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use health_fhir_search::{Catalog, ParamState, Registry, Unrestricted};
use health_fhir_server::{AUDIT_EVENT_INTERACTIONS, FhirServer, TYPE_INTERACTIONS};
use health_identifiers::ResourceId;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const BASE: &str = "http://test/fhir";

fn registry() -> Registry {
    let bundle: Value = serde_json::from_str(include_str!(
        "../../../contracts/health-fhir-search/tests/fixtures/search-parameters.json"
    ))
    .expect("fixture");
    let catalog = Catalog::from_definitions(
        bundle["entry"]
            .as_array()
            .expect("entries")
            .iter()
            .map(|e| &e["resource"]),
    );
    let mut registry = Registry::new(jiff::tz::Offset::from_hours(3).expect("offset"));
    for (resource_type, codes) in [
        (
            "Patient",
            &[
                "_id",
                "_lastUpdated",
                "name",
                "family",
                "birthdate",
                "gender",
                "identifier",
            ][..],
        ),
        (
            "Observation",
            &["_id", "code", "subject", "date", "status"][..],
        ),
    ] {
        for code in codes {
            registry
                .support(&catalog, resource_type, code, ParamState::Active)
                .expect(code);
        }
    }
    registry
}

async fn router() -> Router {
    router_with(None).await
}

/// A server that audits, as every runtime's does.
async fn audited_router() -> Router {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    FhirServer::new(store, BASE, Arc::new(Unrestricted))
        .with_audit(health_audit::Auditor::new("test node", Vec::new()))
        .router()
}

/// AuditEvent is read-only over REST: its declared reads work on the events the earlier writes
/// produced, and every write is refused.
async fn audit_event_is_read_only(router: &Router) {
    let json = vec![(
        "content-type".to_owned(),
        "application/fhir+json".to_owned(),
    )];
    let search = send(router, "GET", "/AuditEvent?_count=1", &[], String::new()).await;
    assert_eq!(search.status, StatusCode::OK, "{}", search.body);
    let bundle: Value = serde_json::from_str(&search.body).expect("json");
    let event = &bundle["entry"][0]["resource"];
    let id = event["id"]
        .as_str()
        .expect("an AuditEvent from an earlier write");
    let vid = event["meta"]["versionId"].as_str().expect("versionId");
    for path in [
        format!("/AuditEvent/{id}"),
        format!("/AuditEvent/{id}/_history/{vid}"),
        format!("/AuditEvent/{id}/_history"),
        "/AuditEvent/_history?_count=1".to_owned(),
    ] {
        let reply = send(router, "GET", &path, &[], String::new()).await;
        assert_eq!(reply.status, StatusCode::OK, "{path}: {}", reply.body);
    }
    let mut changed = event.clone();
    changed["action"] = Value::String("D".into());
    let attempts = [
        ("PUT", format!("/AuditEvent/{id}"), changed.to_string()),
        ("POST", "/AuditEvent".to_owned(), changed.to_string()),
        ("DELETE", format!("/AuditEvent/{id}"), String::new()),
    ];
    for (method, path, body) in attempts {
        let reply = send(router, method, &path, &json, body).await;
        assert_eq!(
            reply.status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path}: {}",
            reply.body
        );
    }
}

async fn router_with(scope: Option<Arc<dyn health_fhir_server::ReferenceScope>>) -> Router {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock)).with_registry(registry());
    let server = FhirServer::new(store, BASE, Arc::new(Unrestricted));
    match scope {
        Some(scope) => server.with_reference_scope(scope).router(),
        None => server.router(),
    }
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: String,
}

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: String,
) -> Reply {
    let mut request = Request::builder()
        .method(Method::from_bytes(method.as_bytes()).expect("method"))
        .uri(path);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::from(body)).expect("request"))
        .await
        .expect("infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    Reply {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).expect("utf-8"),
    }
}

struct Vars(BTreeMap<String, String>);

impl Vars {
    fn substitute(&mut self, text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let end = rest[start..].find("}}").expect("closing }}") + start;
            let name = &rest[start + 2..end];
            let value = if let Some(fresh) = name.strip_prefix("uuid:") {
                self.0
                    .entry(format!("uuid:{fresh}"))
                    .or_insert_with(|| ResourceId::new().to_string())
                    .clone()
            } else {
                self.0
                    .get(name)
                    .unwrap_or_else(|| panic!("unset {{{{{name}}}}}"))
                    .clone()
            };
            out.push_str(&value);
            rest = &rest[end + 2..];
        }
        out.push_str(rest);
        out
    }
}

async fn run_fixture(router: &Router, fixture: &Value) {
    let name = fixture["name"].as_str().expect("name");
    let mut vars = Vars(BTreeMap::new());
    for (index, step) in fixture["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .enumerate()
    {
        let at = format!("{name} step {index}");
        if let Some(ms) = step["sleep_ms"].as_u64() {
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            continue;
        }
        let request = &step["request"];
        let method = request["method"].as_str().expect("method");
        let path = match request["url_from"].as_str() {
            Some(var) => vars.0[var]
                .strip_prefix(BASE)
                .expect("absolute link")
                .to_owned(),
            None => format!(
                "/{}",
                vars.substitute(request["path"].as_str().expect("path"))
            ),
        };
        let headers: Vec<(String, String)> = request["headers"]
            .as_object()
            .map(|h| {
                h.iter()
                    .map(|(k, v)| (k.clone(), vars.substitute(v.as_str().expect("header"))))
                    .collect()
            })
            .unwrap_or_default();
        let body = match (&request["body"], request["form"].as_str()) {
            (Value::Null, Some(form)) => vars.substitute(form),
            (Value::Null, None) => String::new(),
            (json, _) => vars.substitute(&json.to_string()),
        };
        let reply = send(router, method, &path, &headers, body).await;

        let expect = &step["expect"];
        assert_eq!(
            u64::from(reply.status.as_u16()),
            expect["status"].as_u64().expect("status"),
            "{at}: {method} {path} → {}",
            reply.body
        );
        for (header, value) in expect["headers"].as_object().into_iter().flatten() {
            let actual = reply.headers.get(header).and_then(|v| v.to_str().ok());
            let wanted = vars.substitute(value.as_str().expect("header value"));
            if wanted == "*" {
                assert!(actual.is_some(), "{at}: missing {header}");
            } else {
                assert_eq!(actual, Some(wanted.as_str()), "{at}: {header}");
            }
        }
        for header in expect["headers_absent"].as_array().into_iter().flatten() {
            let header = header.as_str().expect("header");
            assert!(
                !reply.headers.contains_key(header),
                "{at}: unexpected {header}"
            );
        }
        if expect["empty_body"] == Value::Bool(true) {
            assert!(reply.body.is_empty(), "{at}: body {}", reply.body);
        }
        for needle in expect["body_contains"].as_array().into_iter().flatten() {
            let needle = needle.as_str().expect("text");
            assert!(
                reply.body.contains(needle),
                "{at}: body lacks {needle}: {}",
                reply.body
            );
        }
        let json: Value = if reply.body.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&reply.body).unwrap_or_else(|e| panic!("{at}: not JSON ({e})"))
        };
        for (pointer, value) in expect["json"].as_object().into_iter().flatten() {
            let wanted = match value {
                Value::String(s) => Value::String(vars.substitute(s)),
                other => other.clone(),
            };
            assert_eq!(
                json.pointer(pointer),
                Some(&wanted),
                "{at}: {pointer} in {json}"
            );
        }
        for (pointer, value) in expect["json_not"].as_object().into_iter().flatten() {
            assert_ne!(json.pointer(pointer), Some(value), "{at}: {pointer}");
        }
        for pointer in expect["json_absent"].as_array().into_iter().flatten() {
            let pointer = pointer.as_str().expect("pointer");
            assert_eq!(json.pointer(pointer), None, "{at}: {pointer} present");
        }
        for (pointer, length) in expect["json_len"].as_object().into_iter().flatten() {
            let actual = json
                .pointer(pointer)
                .and_then(Value::as_array)
                .map(Vec::len);
            let wanted = usize::try_from(length.as_u64().expect("length")).expect("usize");
            assert_eq!(actual, Some(wanted), "{at}: length of {pointer}");
        }
        for (var, pointer) in step["save"].as_object().into_iter().flatten() {
            let value = json
                .pointer(pointer.as_str().expect("pointer"))
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{at}: nothing at {pointer} to save"));
            vars.0.insert(var.clone(), value.to_owned());
        }
    }
}

#[tokio::test]
async fn r5_rest_conformance_fixtures_pass() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rest");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("fixtures")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 11, "{files:?}");
    let router = router().await;
    for file in files {
        let fixture: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");
        run_fixture(&router, &fixture).await;
    }
}

/// An interaction, its request (method, path, headers, body) and its expected status.
type Check<'a> = (
    &'a str,
    &'a str,
    String,
    &'a [(String, String)],
    String,
    StatusCode,
);

/// Exercises every interaction the CapabilityStatement declares on every declared type, and
/// every declared search parameter, so a declaration without behaviour fails here.
#[tokio::test]
async fn the_capability_statement_matches_behaviour() {
    let router = audited_router().await;
    let metadata = send(&router, "GET", "/metadata", &[], String::new()).await;
    assert_eq!(metadata.status, StatusCode::OK);
    let statement: Value = serde_json::from_str(&metadata.body).expect("json");
    let rest = &statement["rest"][0];
    let json = vec![(
        "content-type".to_owned(),
        "application/fhir+json".to_owned(),
    )];
    let resources = rest["resource"].as_array().expect("resources");
    assert_eq!(resources.len(), 84);
    for resource in resources {
        let t = resource["type"].as_str().expect("type");
        let codes: Vec<&str> = resource["interaction"]
            .as_array()
            .expect("interactions")
            .iter()
            .map(|i| i["code"].as_str().expect("code"))
            .collect();
        if t == "AuditEvent" {
            assert_eq!(codes, AUDIT_EVENT_INTERACTIONS);
            assert_eq!(resource["conditionalDelete"], "not-supported");
            audit_event_is_read_only(&router).await;
            continue;
        }
        assert_eq!(codes, TYPE_INTERACTIONS, "{t}");
        assert_eq!(resource["conditionalCreate"], true);
        assert_eq!(resource["conditionalUpdate"], true);
        assert_eq!(resource["conditionalDelete"], "single");

        let id = ResourceId::new();
        // A minimal valid instance: every required element, placeholder values (tier 1).
        let mut skeleton = health_fhir_validation::skeleton(t).expect("skeleton");
        let body_without_id = skeleton.to_string();
        skeleton["id"] = Value::String(id.to_string());
        let body = skeleton.to_string();
        let created = send(&router, "PUT", &format!("/{t}/{id}"), &json, body.clone()).await;
        assert_eq!(
            created.status,
            StatusCode::CREATED,
            "update-as-create {t}: {}",
            created.body
        );
        let etag = created.headers["etag"].to_str().expect("etag").to_owned();
        let vid = etag.trim_start_matches("W/").trim_matches('"').to_owned();
        let mut with_etag = json.clone();
        with_etag.push(("if-match".into(), etag));
        let checks: [Check<'_>; 7] = [
            (
                "read",
                "GET",
                format!("/{t}/{id}"),
                &[],
                String::new(),
                StatusCode::OK,
            ),
            (
                "vread",
                "GET",
                format!("/{t}/{id}/_history/{vid}"),
                &[],
                String::new(),
                StatusCode::OK,
            ),
            (
                "update",
                "PUT",
                format!("/{t}/{id}"),
                &with_etag,
                body,
                StatusCode::OK,
            ),
            (
                "history-instance",
                "GET",
                format!("/{t}/{id}/_history"),
                &[],
                String::new(),
                StatusCode::OK,
            ),
            (
                "history-type",
                "GET",
                format!("/{t}/_history?_count=1"),
                &[],
                String::new(),
                StatusCode::OK,
            ),
            (
                "search-type",
                "GET",
                format!("/{t}?_count=1"),
                &[],
                String::new(),
                StatusCode::OK,
            ),
            (
                "create",
                "POST",
                format!("/{t}"),
                &json,
                body_without_id.clone(),
                StatusCode::CREATED,
            ),
        ];
        for (code, method, path, headers, body, status) in checks {
            assert!(codes.contains(&code));
            let reply = send(&router, method, &path, headers, body).await;
            assert_eq!(reply.status, status, "{t} {code}: {}", reply.body);
        }
        let deleted = send(&router, "DELETE", &format!("/{t}/{id}"), &[], String::new()).await;
        assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{t} delete");

        // Conditional interactions need a search parameter to state the condition with.
        let declares_id = resource["searchParam"]
            .as_array()
            .expect("searchParam")
            .iter()
            .any(|p| p["name"] == "_id");
        if declares_id {
            // create mints the id, so the second create's condition names the minted one.
            let unused = ResourceId::new();
            let body = body_without_id.clone();
            let condition = |id: &str| {
                let mut headers = json.clone();
                headers.push(("if-none-exist".into(), format!("_id={id}")));
                headers
            };
            let created = send(
                &router,
                "POST",
                &format!("/{t}"),
                &condition(&unused.to_string()),
                body.clone(),
            )
            .await;
            assert_eq!(
                created.status,
                StatusCode::CREATED,
                "{t} conditional create"
            );
            let minted: Value = serde_json::from_str(&created.body).expect("json");
            let id = minted["id"].as_str().expect("id").to_owned();
            let again = send(
                &router,
                "POST",
                &format!("/{t}"),
                &condition(&id),
                body.clone(),
            )
            .await;
            assert_eq!(
                again.status,
                StatusCode::OK,
                "{t} conditional create, one match"
            );
            let etag = again.headers["etag"].to_str().expect("etag").to_owned();
            let mut with_etag = json.clone();
            with_etag.push(("if-match".into(), etag));
            let updated = send(&router, "PUT", &format!("/{t}?_id={id}"), &with_etag, body).await;
            assert_eq!(updated.status, StatusCode::OK, "{t} conditional update");
            let deleted = send(
                &router,
                "DELETE",
                &format!("/{t}?_id={id}"),
                &[],
                String::new(),
            )
            .await;
            assert_eq!(
                deleted.status,
                StatusCode::NO_CONTENT,
                "{t} conditional delete"
            );
        }

        for param in resource["searchParam"].as_array().expect("searchParam") {
            let name = param["name"].as_str().expect("name");
            let reply = send(
                &router,
                "GET",
                &format!("/{t}?{name}:missing=true"),
                &[],
                String::new(),
            )
            .await;
            assert_eq!(reply.status, StatusCode::OK, "{t}?{name}: {}", reply.body);
        }
    }
    let system_codes: Vec<&str> = rest["interaction"]
        .as_array()
        .expect("interactions")
        .iter()
        .map(|i| i["code"].as_str().expect("code"))
        .collect();
    assert_eq!(system_codes, ["transaction", "batch", "history-system"]);
    let system = send(&router, "GET", "/_history?_count=3", &[], String::new()).await;
    assert_eq!(system.status, StatusCode::OK);
    for kind in ["transaction", "batch"] {
        let bundle = format!(
            r#"{{"resourceType":"Bundle","type":"{kind}","entry":[{{"resource":{{"resourceType":"Patient"}},"request":{{"method":"POST","url":"Patient"}}}}]}}"#
        );
        let reply = send(&router, "POST", "/", &json, bundle).await;
        assert_eq!(reply.status, StatusCode::OK, "{kind}: {}", reply.body);
    }
}

/// Two concurrent conditional creates under the same condition make one resource.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_conditional_creates_make_one_resource() {
    let router = router().await;
    let headers = vec![
        (
            "content-type".to_owned(),
            "application/fhir+json".to_owned(),
        ),
        (
            "if-none-exist".to_owned(),
            "identifier=urn:ug:nin|RACE".to_owned(),
        ),
    ];
    let body =
        r#"{"resourceType":"Patient","identifier":[{"system":"urn:ug:nin","value":"RACE"}]}"#;
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let (router, headers) = (router.clone(), headers.clone());
        tasks.push(tokio::spawn(async move {
            send(&router, "POST", "/Patient", &headers, body.to_owned())
                .await
                .status
        }));
    }
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.expect("task"));
    }
    let created = statuses
        .iter()
        .filter(|s| **s == StatusCode::CREATED)
        .count();
    let found = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!((created, found), (1, 7), "{statuses:?}");
    let search = send(
        &router,
        "GET",
        "/Patient?identifier=urn:ug:nin|RACE",
        &[],
        String::new(),
    )
    .await;
    let bundle: Value = serde_json::from_str(&search.body).expect("json");
    assert_eq!(bundle["total"], 1);
}

/// A node whose scope excludes Encounters: another facility's Encounter may be referenced
/// without being stored here (persistence.md § Reference integrity, out-of-scope row).
struct EncountersOutOfScope;

impl health_fhir_server::ReferenceScope for EncountersOutOfScope {
    fn must_resolve(
        &self,
        target_type: &health_persistence::ResourceType,
        _id: ResourceId,
    ) -> bool {
        target_type.as_str() != "Encounter"
    }
}

#[tokio::test]
async fn out_of_scope_targets_may_be_unresolved() {
    let router = router_with(Some(Arc::new(EncountersOutOfScope))).await;
    let json = vec![(
        "content-type".to_owned(),
        "application/fhir+json".to_owned(),
    )];
    let referral = |target: String| {
        format!(
            r#"{{"resourceType":"Observation","status":"final","code":{{"text":"x"}},"encounter":{{"reference":"{target}"}}}}"#
        )
    };
    let elsewhere = send(
        &router,
        "POST",
        "/Observation",
        &json,
        referral(format!("Encounter/{}", ResourceId::new())),
    )
    .await;
    assert_eq!(elsewhere.status, StatusCode::CREATED, "{}", elsewhere.body);
    let in_scope = send(
        &router,
        "POST",
        "/Observation",
        &json,
        referral(format!("Patient/{}", ResourceId::new())),
    )
    .await;
    assert_eq!(
        in_scope.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        in_scope.body
    );
}
