//! The clinical API on the edge (#106; deployment-topology.md § Mode D): what LAN browsers use
//! to register, check in, triage and consult with the WAN down, over the edge's own store.
//!
//! - **`/api/fhir`** (the client's FHIR base on an edge), **`/api/reception`**,
//!   **`/api/consultation`** and **`/api/notes`**, behind the device gate (an enrolled device's
//!   DPoP proof) and the subject layer (the signed-in user's permissions for the facility).
//! - **The rules are central's,** from the provisioning bundle: the role → permission mappings
//!   turn each user's snapshot into permissions, and the sensitivity labels restrict records in
//!   the policy decision point, as at central. They are read per request, and the routes are
//!   rebuilt when a new bundle brings different rules. Before any bundle the API answers 503.
//! - **Registration** (`Patient/$register`), with `HEALTH_NODE_CODE` and
//!   `HEALTH_REGISTRY_NIN_SYSTEM`: the same matching as central, and MRNs minted under this node's
//!   own code, so they never collide with another node's.
//! - **Search** uses the parameters central does (`health_fhir_server::search_registry`), and a
//!   submitted form is re-validated against its Questionnaire and extracted (#103), with the
//!   definitions central sends down.

use std::sync::{Arc, RwLock};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use health_access_policy::Pdp;
use health_audit::Auditor;
use health_auth::{
    AccessVerifier, AuthStore as _, LocalConfig, Subjects, SurrealAuthStore, subject_layer,
};
use health_client_registry::matching::MatchingRules;
use health_fhir_search::Unrestricted;
use health_fhir_server::{FacilityTag, FhirServer, search_registry};
use health_identifiers::{NumberFormat, ShortCode};
use health_registries_app::fhir::{ECID_SYSTEM, RegistryConfig};
use health_registries_app::matching::RegistryBlocking;
use health_registries_app::register::MrnSettings;
use health_surrealdb::{Connection, SurrealStore};
use health_time::SystemClock;
use tower::ServiceExt as _;

/// The clinical routes, built from the provisioning bundle's rules.
#[derive(Clone)]
pub(crate) struct Clinical {
    store: SurrealStore,
    auth: SurrealAuthStore,
    verifier: AccessVerifier<SurrealAuthStore>,
    /// The FHIR base the server names in its links, e.g. `https://edge.local/api/fhir`.
    base: String,
    node: String,
    /// Registration (#106): the national ID system and this node's MRN code, when configured.
    registry: Option<(String, ShortCode)>,
    built: Arc<RwLock<Option<(LocalConfig, Router)>>>,
}

/// The client registry's configuration and matching rules for `nin`.
fn registry_rules(nin: &str) -> (RegistryConfig, MatchingRules) {
    (
        RegistryConfig {
            nin_system: nin.to_owned(),
        },
        MatchingRules::platform_default(nin, &[nin, ECID_SYSTEM]),
    )
}

impl Clinical {
    /// The clinical API over `connection`, authenticating with `verifier` against `auth`.
    pub(crate) fn new(
        connection: &Connection,
        auth: SurrealAuthStore,
        verifier: AccessVerifier<SurrealAuthStore>,
        origin: &str,
        node: &str,
        registry: Option<(String, ShortCode)>,
    ) -> Self {
        let store = SurrealStore::new(connection.clone(), Arc::new(SystemClock))
            .with_registry(search_registry());
        // With the registry, source Patients carry their blocking keys, as at central.
        let store = match &registry {
            Some((nin, _)) => {
                let (config, rules) = registry_rules(nin);
                store.with_blocking(Arc::new(RegistryBlocking::new(rules, config)))
            }
            None => store,
        };
        Self {
            store,
            auth,
            verifier,
            base: format!("{origin}/api/fhir"),
            node: node.to_owned(),
            registry,
            built: Arc::new(RwLock::new(None)),
        }
    }

    /// The routes for `config`'s rules.
    fn build(&self, config: &LocalConfig) -> Router {
        let sensitive: Vec<(String, String)> = config
            .labels
            .iter()
            .map(|l| (l.system.clone(), l.code.clone()))
            .collect();
        // The edge's own scope: facility records, and the shared records it holds (ADR 0057).
        let fhir = FhirServer::new(
            self.store.clone(),
            self.base.clone(),
            Arc::new(Unrestricted),
        )
        .with_access_control(Pdp::new(config.label_rules()), Arc::new(FacilityTag))
        .with_audit(Auditor::new(self.node.clone(), sensitive))
        .with_questionnaire_checks()
        .with_extraction();
        // Registration with this node's own MRNs, which never collide with another node's.
        let fhir = match &self.registry {
            Some((nin, node)) => {
                let (config, rules) = registry_rules(nin);
                fhir.with_client_registry(config, rules)
                    .with_registration(MrnSettings {
                        node: node.clone(),
                        format: NumberFormat {
                            padding: 6,
                            ..NumberFormat::default()
                        },
                        system: "urn:health:mrn".into(),
                    })
            }
            None => fhir,
        };
        let subjects = Subjects::new(self.verifier.clone(), config.roles.clone());
        let layer =
            axum::middleware::from_fn_with_state(subjects, subject_layer::<SurrealAuthStore>);
        Router::new()
            .nest("/api/fhir", fhir.clone().router())
            .nest("/api/reception", fhir.reception_router())
            .nest("/api/consultation", fhir.consultation_router())
            .nest("/api/notes", fhir.notes_router())
            .layer(layer)
    }

    /// The routes for the rules stored now; `None` before any bundle.
    async fn current(&self) -> Result<Option<Router>, String> {
        let Some(config) = self.auth.local_config().await.map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let held = self
            .built
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|(rules, _)| *rules == config)
            .map(|(_, router)| router.clone());
        if let Some(router) = held {
            return Ok(Some(router));
        }
        let router = self.build(&config);
        *self
            .built
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((config, router.clone()));
        Ok(Some(router))
    }

    /// `/api/fhir`, `/api/reception`, `/api/consultation` and `/api/notes`, to be put behind
    /// the device gate.
    pub(crate) fn routes(self) -> Router {
        let mut router = Router::new();
        for prefix in [
            "/api/fhir",
            "/api/reception",
            "/api/consultation",
            "/api/notes",
        ] {
            router = router
                .route(prefix, any(serve))
                .route(&format!("{prefix}/{{*rest}}"), any(serve));
        }
        router.with_state(self)
    }
}

async fn serve(State(clinical): State<Clinical>, request: Request) -> Response {
    let router = match clinical.current().await {
        Ok(Some(router)) => router,
        Ok(None) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "this edge is not provisioned yet: import central's bundle",
            )
                .into_response();
        }
        Err(error) => {
            tracing::error!(%error, "the clinical API cannot read its rules");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };
    // A fresh request: this route's own `{*rest}` must not reach the clinical routes'
    // extractors.
    let (parts, body) = request.into_parts();
    let mut fresh = Request::builder()
        .method(parts.method)
        .uri(parts.uri)
        .version(parts.version);
    if let Some(headers) = fresh.headers_mut() {
        *headers = parts.headers;
    }
    let Ok(fresh) = fresh.body(Body::new(body)) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match router.oneshot(fresh).await {
        Ok(response) => response,
        Err(infallible) => match infallible {},
    }
}
