//! Every route central serves, in one matrix (#140): with every optional router mounted, each
//! route is checked twice. Without credentials it refuses, and for a signed-in user whose roles
//! carry no permissions it serves nothing. The inventory is checked against the routers' source,
//! so a new route that is not added here fails the test.

use std::collections::BTreeSet;

use health_identity::snapshot::{PermissionSnapshot, RoleAssignment};
use health_identity::{AccountId, AccountStatus, Lockout, LoginAccount};
use health_surrealdb::{MIGRATIONS, Settings, Target, migrate};

use super::tests::{form, send, settings};
use crate::auth::*;

/// How a route is reached.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gate {
    /// Open to anyone: discovery, sign-in, and what carries its own credential (a one-time
    /// code, the node's signature).
    Public,
    /// A signed-in member of staff, with the permissions of their roles.
    Staff,
    /// A patient signed in to the portal.
    Patient,
    /// A provisioned node, by its signature on each request.
    Node,
}

/// One router: the file its routes are declared in, where it is mounted, how it is reached, and
/// its paths as declared.
struct Mounted {
    file: &'static str,
    prefix: &'static str,
    gate: Gate,
    paths: &'static [&'static str],
}

const FHIR: &str = "crates/infrastructure/health-fhir-server/src/";
const AUTH: &str = "crates/infrastructure/health-auth/src/";
const SYNC: &str = "crates/infrastructure/health-sync-server/src/";
const PROVISIONING: &str = "crates/infrastructure/health-provisioning/src/";

/// Every router central mounts (`routes`), with each route it declares.
const INVENTORY: &[Mounted] = &[
    Mounted {
        file: "health-fhir-server/src/lib.rs",
        prefix: "/fhir",
        gate: Gate::Public,
        paths: &["/metadata"],
    },
    Mounted {
        file: "health-fhir-server/src/lib.rs",
        prefix: "/fhir",
        gate: Gate::Staff,
        paths: &[
            "/",
            "/_history",
            "/{type}",
            "/ValueSet/$expand",
            "/ValueSet/$validate-code",
            "/CodeSystem/$validate-code",
            "/CodeSystem/$lookup",
            "/CodeSystem/$subsumes",
            "/ConceptMap/$translate",
            "/QuestionnaireResponse/$extract",
            "/Questionnaire/{id}/$populate",
            "/Patient/$match",
            "/Patient/$register",
            "/Patient/$merge",
            "/Patient/{id}/$summary",
            "/Patient/{id}/$timeline",
            "/Patient/{id}/$journey",
            "/Patient/{id}/$link",
            "/Patient/{id}/$unlink",
            "/{type}/_search",
            "/{type}/_history",
            "/{type}/{id}",
            "/{type}/{id}/_history",
            "/{type}/{id}/_history/{vid}",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/care_plans.rs",
        prefix: "/api/care-plans",
        gate: Gate::Staff,
        paths: &[
            "/plans",
            "/plans/{id}/status",
            "/plans/{id}/addresses",
            "/plans/{id}/activities",
            "/patients/{id}/plans",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/consultation.rs",
        prefix: "/api/consultation",
        gate: Gate::Staff,
        paths: &[
            "/encounters/{id}/notes",
            "/encounters/{id}/diagnoses",
            "/encounters/{id}/orders",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/dispensing.rs",
        prefix: "/api/pharmacy",
        gate: Gate::Staff,
        paths: &[
            "/prescriptions/{id}/prepare",
            "/dispenses/{id}",
            "/dispenses/{id}/complete",
            "/dispenses/{id}/cancel",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/pharmacy.rs",
        prefix: "/api/pharmacy",
        gate: Gate::Staff,
        paths: &["/worklist", "/prescriptions/{id}/status"],
    },
    Mounted {
        file: "health-fhir-server/src/inbox.rs",
        prefix: "/api/inbox",
        gate: Gate::Staff,
        paths: &["/"],
    },
    Mounted {
        file: "health-fhir-server/src/inventory.rs",
        prefix: "/api/stock",
        gate: Gate::Staff,
        paths: &[
            "/goods-received",
            "/on-hand",
            "/reorder",
            "/gs1",
            "/requisitions",
            "/requisitions/{id}/decision",
            "/requisitions/{id}/fulfil",
            "/products",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/lab.rs",
        prefix: "/api/laboratory",
        gate: Gate::Staff,
        paths: &[
            "/tests/{id}",
            "/tests/{id}/collect",
            "/tests/{id}/result",
            "/tests/{id}/verify",
            "/tests/{id}/review",
            "/tests/{id}/reject",
            "/tests/{id}/cancel",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/notes.rs",
        prefix: "/api/notes",
        gate: Gate::Staff,
        paths: &[
            "/notes",
            "/notes/{id}/edit",
            "/notes/{id}/cancel",
            "/notes/{id}/sign",
            "/notes/{id}/amend",
            "/patients/{id}/notes",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/payments.rs",
        prefix: "/api/payments",
        gate: Gate::Staff,
        paths: &[
            "/shifts",
            "/shifts/open",
            "/shifts/{id}",
            "/shifts/{id}/close",
            "/shifts/{id}/approve",
            "/cash",
            "/pending",
            "/payments/{id}",
            "/payments/{id}/void",
            "/payments/{id}/refund",
            "/statements",
            "/balances",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/pricing.rs",
        prefix: "/api/pricing",
        gate: Gate::Staff,
        paths: &[
            "/books",
            "/books/{id}/{version}/{step}",
            "/rules",
            "/rules/{id}/{version}/{step}",
            "/quote",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/programs.rs",
        prefix: "/api/programs",
        gate: Gate::Staff,
        paths: &[
            "/definitions",
            "/enrolments",
            "/patients/{id}/enrolments",
            "/enrolments/{id}/status",
            "/enrolments/{id}/transfer",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/reception.rs",
        prefix: "/api/reception",
        gate: Gate::Staff,
        paths: &[
            "/check-in",
            "/encounters/{id}/move",
            "/encounters/{id}/status",
            "/queues/{service_point}",
            "/queue/{entry}/{step}",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/registry.rs",
        prefix: "/api/registry/reviews",
        gate: Gate::Staff,
        paths: &["/", "/{id}"],
    },
    Mounted {
        file: "health-fhir-server/src/requests.rs",
        prefix: "/api/requests",
        gate: Gate::Staff,
        paths: &["/{kind}/{id}/accept", "/{kind}/{id}/decline"],
    },
    Mounted {
        file: "health-fhir-server/src/requests.rs",
        prefix: "/api/portal",
        gate: Gate::Patient,
        paths: &[
            "/requests",
            "/requests/{kind}/{id}/withdraw",
            "/prescriptions",
        ],
    },
    Mounted {
        file: "health-fhir-server/src/portal.rs",
        prefix: "/api/portal",
        gate: Gate::Patient,
        paths: &["/registration"],
    },
    Mounted {
        file: "health-fhir-server/src/workflow.rs",
        prefix: "/api/workflow",
        gate: Gate::Staff,
        paths: &[
            "/instances",
            "/instances/{id}/tasks",
            "/instances/{id}/events",
            "/instances/{id}/cancel",
            "/instances/{id}/tasks/{state}/{command}",
            "/worklists/{key}",
            "/definitions",
        ],
    },
    Mounted {
        file: "health-sync-server/src/lib.rs",
        prefix: "",
        gate: Gate::Node,
        paths: &[
            "/sync/v1/hello",
            "/sync/v1/push",
            "/sync/v1/pull",
            "/sync/v1/snapshot",
            "/sync/v1/blobs/{hash}/{size}",
            "/sync/v1/blobs/{hash}/{size}/{offset}",
            "/sync/v1/blobs/{hash}/content",
            "/sync/v1/break-glass",
            "/sync/v1/transfers",
            "/sync/v1/transfers/inbound",
            "/sync/v1/activation-reports",
            "/sync/v1/events",
        ],
    },
    Mounted {
        file: "health-sync-server/src/review.rs",
        prefix: "/api/sync/conflicts",
        gate: Gate::Staff,
        paths: &["/", "/{id}"],
    },
    Mounted {
        file: "health-auth/src/subject.rs",
        prefix: "/api",
        gate: Gate::Staff,
        paths: &[
            "/break-glass",
            "/break-glass/alerts",
            "/break-glass/alerts/{id}/acknowledge",
            "/break-glass/reviews",
            "/break-glass/reviews/{id}",
            "/events",
        ],
    },
    Mounted {
        file: "health-auth/src/caregivers.rs",
        prefix: "/api/caregivers",
        gate: Gate::Staff,
        paths: &["/", "/{id}/revoke", "/{id}/adolescent-consent"],
    },
    Mounted {
        file: "health-auth/src/caregivers.rs",
        prefix: "/api/portal",
        gate: Gate::Patient,
        paths: &["/dependents"],
    },
    Mounted {
        file: "health-auth/src/server.rs",
        prefix: "/auth",
        gate: Gate::Public,
        paths: &[
            "/.well-known/openid-configuration",
            "/.well-known/smart-configuration",
            "/jwks",
            "/authorize",
            "/otp",
            "/token",
            "/revoke",
            // Each carries its own credential: the current password, or the PIN with the
            // device's DPoP proof.
            "/password",
            "/unlock",
        ],
    },
    Mounted {
        file: "health-auth/src/server.rs",
        prefix: "/auth",
        gate: Gate::Staff,
        paths: &["/pin"],
    },
    Mounted {
        file: "health-auth/src/server.rs",
        prefix: "/api",
        gate: Gate::Staff,
        paths: &["/session"],
    },
    Mounted {
        file: "health-provisioning/src/routes.rs",
        prefix: "/api",
        gate: Gate::Public,
        paths: &["/provisioning/nodes", "/provisioning/recovery"],
    },
];

/// Files in those crates whose routes central does not mount, and why.
const NOT_AT_CENTRAL: &[(&str, &str)] = &[
    (
        "health-auth/src/provisioning.rs",
        "the edge imports its account bundle (behind its local administrator)",
    ),
    (
        "health-auth/src/local_admin.rs",
        "the edge's local administrator",
    ),
    ("health-auth/src/devices.rs", "the edge's device enrolment"),
];

/// The routers' source files, beside the inventory's short names.
const SOURCES: &[(&str, &str)] = &[
    ("health-fhir-server/src/", FHIR),
    ("health-auth/src/", AUTH),
    ("health-sync-server/src/", SYNC),
    ("health-provisioning/src/", PROVISIONING),
];

/// The paths a source file declares with `.route("…"`.
fn declared(source: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let mut rest = source;
    while let Some(at) = rest.find(".route(") {
        rest = &rest[at + ".route(".len()..];
        let trimmed = rest.trim_start();
        if let Some(literal) = trimmed.strip_prefix('"')
            && let Some(end) = literal.find('"')
        {
            paths.insert(literal[..end].to_owned());
        }
    }
    paths
}

/// The source of a file named as in the inventory, with its `#[cfg(test)]` modules left out.
fn source(file: &str) -> String {
    let (short, long) = SOURCES
        .iter()
        .find(|(short, _)| file.starts_with(short))
        .expect("a known crate");
    let path = format!(
        "{}/../../../{long}{}",
        env!("CARGO_MANIFEST_DIR"),
        &file[short.len()..]
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    match text.find("#[cfg(test)]") {
        Some(at) => text[..at].to_owned(),
        None => text,
    }
}

#[test]
fn the_inventory_holds_every_route_the_routers_declare() {
    let files: BTreeSet<&str> = INVENTORY.iter().map(|m| m.file).collect();
    for file in files {
        let listed: BTreeSet<String> = INVENTORY
            .iter()
            .filter(|m| m.file == file)
            .flat_map(|m| m.paths.iter().map(|p| (*p).to_owned()))
            .collect();
        assert_eq!(declared(&source(file)), listed, "{file}");
    }
    // Every file in those crates that declares a route is in the inventory.
    for (short, long) in SOURCES {
        let dir = format!("{}/../../../{long}", env!("CARGO_MANIFEST_DIR"));
        for entry in std::fs::read_dir(&dir).expect("dir") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                let file = format!(
                    "{short}{}",
                    path.file_name().expect("name").to_string_lossy()
                );
                if !declared(&source(&file)).is_empty() {
                    assert!(
                        INVENTORY.iter().any(|m| m.file == file)
                            || NOT_AT_CENTRAL.iter().any(|(f, _)| *f == file),
                        "{file} declares routes the inventory does not list"
                    );
                }
            }
        }
    }
}

/// A concrete path for a declared one.
fn concrete(prefix: &str, path: &str) -> String {
    let path = path
        .replace("{type}", "Patient")
        .replace(
            "{hash}",
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .replace("{size}", "10")
        .replace("{offset}", "0")
        .replace("{vid}", "1")
        .replace("{version}", "1.0.0")
        .replace("{step}", "submit")
        .replace("{kind}", "ServiceRequest")
        .replace("{state}", "triage")
        .replace("{command}", "claim")
        .replace("{service_point}", "triage")
        .replace("{entry}", "018f0000-0000-7000-8000-000000000001")
        .replace("{key}", "triage")
        .replace("{id}", "018f0000-0000-7000-8000-000000000000");
    match path.as_str() {
        "/" => prefix.to_owned(),
        _ => format!("{prefix}{path}"),
    }
}

const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];

fn request(method: &str, path: &str, bearer: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-purpose-of-use", "TREAT");
    if let Some(bearer) = bearer {
        request = request.header("authorization", bearer);
    }
    if method == "GET" || method == "DELETE" {
        request.body(Body::empty()).expect("request")
    } else {
        request
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .expect("request")
    }
}

/// Central with every optional router mounted, and a bearer token for a signed-in member of
/// staff whose only role carries no permissions.
async fn central() -> (Router, String, tempfile::TempDir) {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let snapshot_key = [7u8; 32];
    let policy: AccessPolicy = serde_json::from_value(serde_json::json!({
        "roles": { "visitor": [] },
        "labels": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "HIV", "program": "hiv" }],
    }))
    .expect("policy");
    let accounts = SurrealAuthStore::new(
        SurrealStore::new(connection.clone(), Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    );
    let credentials = Credentials::new(Vec::new()).expect("credentials");
    let account = AccountId(uuid::Uuid::now_v7());
    accounts
        .put_account(&LoginAccount {
            id: account,
            username: "visitor".into(),
            credential: credentials
                .credential(account, "correct horse battery")
                .expect("hash"),
            status: AccountStatus::Active,
            lockout: Lockout::default(),
        })
        .await
        .expect("account");
    let issued = health_time::Timestamp::now();
    let snapshot = PermissionSnapshot {
        account,
        facility: "KAM013".into(),
        roles: vec![RoleAssignment {
            role: "visitor".into(),
            locations: vec![],
        }],
        programs: vec![],
        issued_at: issued,
        not_after: SnapshotPolicy::default().not_after(issued),
        snapshot_version: 1,
    };
    let jws = SnapshotSigner::from_secret(&snapshot_key)
        .expect("key")
        .sign(&snapshot, &policy.roles)
        .expect("sign");
    accounts
        .put_snapshot(account, "KAM013", &jws)
        .await
        .expect("snapshot");

    let dir = tempfile::tempdir().expect("dir");
    let mut settings = settings();
    settings.login_page = None;
    settings.snapshot_keys = vec![snapshot_key];
    settings.registry_nin_system = Some("urn:health:nin".into());
    settings.policy = Some(policy);
    settings.portal = Some(PortalSettings {
        country_code: "256".into(),
        otp_outbox: dir.path().join("otp"),
    });
    settings.payments = Some(health_fhir_server::PaymentConfig {
        currency: health_financial_ledger::Currency::from_code("UGX").expect("currency"),
        tills: vec!["till-1".into()],
        node: "C1".into(),
        pending_for: health_time::SignedDuration::from_hours(24),
    });
    settings.issuing_ca = Some(Arc::new(
        health_pki::Authority::root("route matrix", health_time::Timestamp::now()).expect("ca"),
    ));
    settings.kms = Some(Arc::new(LocalKeyWrap::new(&[7; 32])));
    settings.files =
        Some(health_files::FsBlobStore::open(dir.path().join("files")).expect("files"));
    settings.workflows =
        Some(health_fhir_server::WorkflowConfig::new(Vec::new(), "C1").expect("workflows"));
    let ready = Arc::new(OnceCell::new());
    let app = gate(Arc::clone(&ready));
    let _ = ready.set(routes(connection, &settings).expect("routes"));

    let (_, _, location) = send(
        &app,
        form(
            "/auth/authorize",
            &[
                ("response_type", "code"),
                ("client_id", "health-client"),
                ("redirect_uri", "https://central.test/callback"),
                ("scope", "openid"),
                ("state", "s"),
                (
                    "code_challenge",
                    "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
                ),
                ("code_challenge_method", "S256"),
                ("username", "visitor"),
                ("password", "correct horse battery"),
            ],
        ),
    )
    .await;
    let location = location.expect("redirect");
    let code = form_urlencoded::parse(location.split_once('?').expect("query").1.as_bytes())
        .find(|(k, _)| k == "code")
        .expect("code")
        .1
        .into_owned();
    let (status, body, _) = send(
        &app,
        form(
            "/auth/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", "https://central.test/callback"),
                ("client_id", "health-client"),
                (
                    "code_verifier",
                    "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                ),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tokens: serde_json::Value = serde_json::from_str(&body).expect("json");
    let bearer = format!("Bearer {}", tokens["access_token"].as_str().expect("token"));
    (app, bearer, dir)
}

/// The status a request is answered with. Streams (the event feeds) are not read to their end.
async fn status(app: &Router, request: Request<Body>) -> u16 {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        app.clone().oneshot(request),
    )
    .await
    .expect("answered in time")
    .expect("response")
    .status()
    .as_u16()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_route_refuses_the_anonymous_and_the_unpermitted() {
    let (app, bearer, _dir) = central().await;
    let mut problems = Vec::new();
    for mounted in INVENTORY {
        for path in mounted.paths {
            let path = concrete(mounted.prefix, path);
            let mut anonymous = Vec::new();
            let mut unpermitted = Vec::new();
            for method in METHODS {
                anonymous.push(status(&app, request(method, &path, None)).await);
                unpermitted.push(status(&app, request(method, &path, Some(&bearer))).await);
            }
            // Mounted: some method reaches the route.
            if anonymous.iter().all(|s| *s == 404) {
                problems.push(format!("{path}: not mounted {anonymous:?}"));
                continue;
            }
            if mounted.gate == Gate::Public {
                continue;
            }
            // Without credentials: 401, before the request is read (405 for another method).
            if !anonymous.iter().all(|s| matches!(s, 401 | 405)) || !anonymous.contains(&401) {
                problems.push(format!("{path}: anonymous {anonymous:?}"));
            }
            let refused = match mounted.gate {
                // A member of staff with no permissions is served nothing, except what is
                // their own or is filtered to what they may see.
                Gate::Staff => {
                    UNPERMITTED_SERVED.iter().any(|(p, _)| *p == path)
                        || unpermitted.iter().all(|s| !(200..300).contains(s))
                }
                // A staff token is not a patient's.
                Gate::Patient => unpermitted.iter().all(|s| matches!(s, 401 | 403)),
                // A user's token is not a node's signature.
                Gate::Node => unpermitted.iter().all(|s| matches!(s, 401 | 405)),
                Gate::Public => true,
            };
            if !refused {
                problems.push(format!("{path}: unpermitted {unpermitted:?}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// What a member of staff whose roles carry no permissions is answered 200 by, and why.
/// Records they may not read answer 404 like unknown ones, so their existence is not disclosed.
const UNPERMITTED_SERVED: &[(&str, &str)] = &[
    ("/api/session", "who the caller is"),
    ("/api/events", "the caller's own change hints"),
    ("/api/inbox", "the caller's own inbox"),
    (
        "/fhir/_history",
        "system history, filtered to the records the caller may list",
    ),
];
