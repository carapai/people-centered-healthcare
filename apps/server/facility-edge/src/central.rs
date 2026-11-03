//! The edge's one call to central before it has an identity: redeeming its provisioning code
//! (#268). HTTPS only. Central's certificate is verified against the operating system's trust
//! store, or, when the installer names one, against that CA alone (an in-country private CA).
//! There is no bundled root store, and no way to skip verification.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt as _, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use serde_json::Value;

/// How long the call may take.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The TLS configuration: `ca_pem` alone when given, else the OS trust store.
fn tls(
    ca_pem: Option<&str>,
) -> Result<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, String>
{
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = hyper_rustls::HttpsConnectorBuilder::new();
    let builder = match ca_pem {
        Some(pem) => {
            let mut roots = RootCertStore::empty();
            for der in CertificateDer::pem_slice_iter(pem.as_bytes()) {
                roots
                    .add(der.map_err(|e| format!("central's CA: {e}"))?)
                    .map_err(|e| format!("central's CA: {e}"))?;
            }
            if roots.is_empty() {
                return Err("central's CA file holds no certificate".into());
            }
            let config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .with_root_certificates(roots)
                .with_no_client_auth();
            builder.with_tls_config(config)
        }
        None => builder
            .with_provider_and_native_roots(provider)
            .map_err(|e| format!("the system trust store: {e}"))?,
    };
    Ok(builder.https_only().enable_http1().build())
}

/// POSTs `body` as JSON to `url`; returns the status and the JSON answer (`null` if none).
pub(crate) fn post_json(
    url: &str,
    body: &Value,
    ca_pem: Option<&str>,
) -> Result<(u16, Value), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let client: Client<_, Full<VecDeque<u8>>> =
            Client::builder(TokioExecutor::new()).build(tls(ca_pem)?);
        let request = hyper::Request::post(url)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .body(Full::new(VecDeque::from(body.to_string().into_bytes())))
            .map_err(|e| format!("{url}: {e}"))?;
        let response = tokio::time::timeout(TIMEOUT, client.request(request))
            .await
            .map_err(|_| format!("{url} did not answer in time"))?
            .map_err(|e| format!("cannot reach {url}: {e:?}"))?;
        let status = response.status().as_u16();
        let bytes = tokio::time::timeout(TIMEOUT, response.into_body().collect())
            .await
            .map_err(|_| format!("{url} did not answer in time"))?
            .map_err(|e| format!("{url}: {e}"))?
            .to_bytes();
        Ok((
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        ))
    })
}
