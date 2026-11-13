//! The provider's API over HTTPS, verified against the operating system's trust store.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt as _, Full};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use serde_json::Value;

use super::mtn::{MomoError, MomoTransport, Request};

/// How long one call may take.
const TIMEOUT: Duration = Duration::from_secs(30);

type Https = hyper_rustls::HttpsConnector<HttpConnector>;

/// The provider's API at `base` (e.g. `https://sandbox.momodeveloper.mtn.com`), over HTTPS only.
#[derive(Clone)]
pub struct HttpsTransport {
    base: Arc<str>,
    client: Client<Https, Full<VecDeque<u8>>>,
}

impl HttpsTransport {
    /// A transport to `base`, its certificate verified against the OS trust store.
    pub fn new(base: &str) -> Result<Self, String> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_provider_and_native_roots(provider)
            .map_err(|e| format!("the system trust store: {e}"))?
            .https_only()
            .enable_http1()
            .build();
        Ok(Self {
            base: base.trim_end_matches('/').into(),
            client: Client::builder(TokioExecutor::new()).build(connector),
        })
    }
}

impl MomoTransport for HttpsTransport {
    async fn send(&self, request: Request) -> Result<(u16, Value), MomoError> {
        let mut builder = hyper::Request::builder()
            .method(request.method)
            .uri(format!("{}{}", self.base, request.path))
            .header("accept", "application/json");
        for (name, value) in &request.headers {
            builder = builder.header(*name, value);
        }
        let body = match &request.body {
            Some(body) => {
                builder = builder.header("content-type", "application/json");
                body.to_string().into_bytes()
            }
            None => Vec::new(),
        };
        let outgoing = builder
            .body(Full::new(VecDeque::from(body)))
            .map_err(|e| MomoError::Unexpected(e.to_string()))?;
        let response = tokio::time::timeout(TIMEOUT, self.client.request(outgoing))
            .await
            .map_err(|_| MomoError::Unreachable)?
            .map_err(|_| MomoError::Unreachable)?;
        let status = response.status().as_u16();
        let bytes = tokio::time::timeout(TIMEOUT, response.into_body().collect())
            .await
            .map_err(|_| MomoError::Unreachable)?
            .map_err(|_| MomoError::Unreachable)?
            .to_bytes();
        Ok((
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        ))
    }
}
