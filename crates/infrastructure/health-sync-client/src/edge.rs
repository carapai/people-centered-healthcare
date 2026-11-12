//! A desktop's requests to its edge (#133; desktop-client.md § Online and offline under an edge):
//! the commands it passes through, and its staff's sign-in there. Each request carries a DPoP
//! proof by the desktop's enrolled device key, bound to the access token when one is sent, so
//! the edge's gate admits the device and its verifier the token (RFC 9449). HTTPS only.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use health_provisioning::{dpop_proof, dpop_proof_with_token};
use health_time::Timestamp;
use http_body_util::{BodyExt as _, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use p256::ecdsa::SigningKey;

use crate::Https;

/// A request to the edge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    /// `GET`, `POST`, …
    pub method: String,
    /// The origin-relative path, without the query.
    pub path: String,
    /// The query string, without `?`, if any.
    pub query: Option<String>,
    /// Headers, other than the credentials this client adds.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

/// The edge's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The HTTP status.
    pub status: u16,
    /// Its headers, a repeated one's values joined with `, `.
    pub headers: Vec<(String, String)>,
    /// Its body.
    pub body: Vec<u8>,
}

/// Why the edge gave no answer: it could not be reached, or did not answer in time. The link
/// counts these towards working offline; an answer, even an error, means the edge is there.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the edge could not be reached: {0}")]
pub struct Unreachable(pub String);

/// The desktop's client of its edge.
#[derive(Clone)]
pub struct Edge {
    origin: Arc<str>,
    key: Arc<SigningKey>,
    client: Client<Https, Full<VecDeque<u8>>>,
    timeout: Duration,
}

impl Edge {
    /// A client of the edge at `origin` (as its `HEALTH_ORIGINS` names it), verified against
    /// `ca_pem` when given, proving requests with the device's `key`; each request may take
    /// `timeout`.
    pub fn new(
        origin: &str,
        ca_pem: Option<&str>,
        key: SigningKey,
        timeout: Duration,
    ) -> Result<Self, String> {
        Ok(Self {
            origin: origin.trim_end_matches('/').into(),
            key: Arc::new(key),
            client: Client::builder(TokioExecutor::new()).build(crate::connector(ca_pem)?),
            timeout,
        })
    }

    /// The edge's origin.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Sends `request`, with `token` (DPoP-bound) when given.
    pub async fn send(&self, request: Request, token: Option<&str>) -> Result<Answer, Unreachable> {
        let htu = format!("{}{}", self.origin, request.path);
        let uri = match &request.query {
            Some(query) if !query.is_empty() => format!("{htu}?{query}"),
            _ => htu.clone(),
        };
        let now = Timestamp::now();
        let mut builder = hyper::Request::builder()
            .method(request.method.as_str())
            .uri(&uri);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        builder = match token {
            Some(token) => builder
                .header("authorization", format!("DPoP {token}"))
                .header(
                    "dpop",
                    dpop_proof_with_token(&request.method, &htu, &self.key, now, token),
                ),
            None => builder.header("dpop", dpop_proof(&request.method, &htu, &self.key, now)),
        };
        let outgoing = builder
            .body(Full::new(VecDeque::from(request.body)))
            .map_err(|e| Unreachable(format!("{uri}: {e}")))?;
        let response = tokio::time::timeout(self.timeout, self.client.request(outgoing))
            .await
            .map_err(|_| Unreachable(format!("{uri} did not answer in time")))?
            .map_err(|e| Unreachable(format!("{uri}: {e:?}")))?;
        let status = response.status().as_u16();
        let mut headers: Vec<(String, String)> = Vec::new();
        for (name, value) in response.headers() {
            let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
            match headers.iter_mut().find(|(n, _)| n == name.as_str()) {
                Some((_, joined)) => {
                    joined.push_str(", ");
                    joined.push_str(&value);
                }
                None => headers.push((name.as_str().to_owned(), value)),
            }
        }
        let body = tokio::time::timeout(self.timeout, response.into_body().collect())
            .await
            .map_err(|_| Unreachable(format!("{uri} did not answer in time")))?
            .map_err(|e| Unreachable(format!("{uri}: {e}")))?
            .to_bytes()
            .to_vec();
        Ok(Answer {
            status,
            headers,
            body,
        })
    }
}
