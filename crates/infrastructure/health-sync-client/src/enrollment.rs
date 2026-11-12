//! A desktop's enrollment with its edge (#133; lan-security.md § Device enrollment; ADR 0009):
//! it asks with its device key, shows the pairing code the edge gives it, and waits for a
//! facility admin to approve it on the edge. Thereafter that key's DPoP proofs are its
//! credential, for the API and for sync ([`crate::Sync::device`]). Blocking, for the desktop's
//! commands; HTTPS only, verified against the facility's chain when given.

use health_provisioning::dpop_proof;
use health_time::Timestamp;
use p256::ecdsa::SigningKey;
use serde_json::{Value, json};

use crate::central::send;

/// Where an enrollment stands, as the edge sees the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enrollment {
    /// Waiting for an admin to type `code` on the edge.
    Pending {
        /// The pairing code to show.
        code: String,
    },
    /// Approved: the device's id on the edge, which is its node id in sync.
    Enrolled {
        /// The device id.
        device: String,
    },
    /// The device was revoked; its key is refused for good.
    Revoked,
    /// No request is pending for the key (none was made, or it expired).
    None,
}

fn call(
    edge: &str,
    method: &str,
    body: &Value,
    key: &SigningKey,
    ca_pem: Option<&str>,
) -> Result<(u16, Value), String> {
    let url = format!("{}/api/enrollment", edge.trim_end_matches('/'));
    let proof = dpop_proof(method, &url, key, Timestamp::now());
    let body = if method == "GET" {
        Vec::new()
    } else {
        body.to_string().into_bytes()
    };
    send(
        method,
        &url,
        &[("dpop", &proof), ("content-type", "application/json")],
        body,
        ca_pem,
    )
}

/// Where `key`'s enrollment at `edge` stands.
pub fn status(edge: &str, key: &SigningKey, ca_pem: Option<&str>) -> Result<Enrollment, String> {
    let (code, answer) = call(edge, "GET", &Value::Null, key, ca_pem)?;
    if code != 200 && code != 404 {
        return Err(refused(code, &answer));
    }
    Ok(match answer["status"].as_str() {
        Some("pending") => Enrollment::Pending {
            code: answer["code"].as_str().unwrap_or_default().to_owned(),
        },
        Some("enrolled") => Enrollment::Enrolled {
            device: answer["device"]["id"]
                .as_str()
                .ok_or("the edge's answer names no device")?
                .to_owned(),
        },
        Some("revoked") => Enrollment::Revoked,
        _ => Enrollment::None,
    })
}

/// Asks `edge` to enroll `key` as `label`; the pairing code to show, or where it already stands.
pub fn request(
    edge: &str,
    label: &str,
    key: &SigningKey,
    ca_pem: Option<&str>,
) -> Result<Enrollment, String> {
    let (code, answer) = call(edge, "POST", &json!({ "label": label }), key, ca_pem)?;
    match code {
        201 => Ok(Enrollment::Pending {
            code: answer["code"]
                .as_str()
                .ok_or("the edge's answer has no pairing code")?
                .to_owned(),
        }),
        // Already enrolled, or revoked: say where it stands.
        409 | 403 => status(edge, key, ca_pem),
        _ => Err(refused(code, &answer)),
    }
}

fn refused(code: u16, answer: &Value) -> String {
    format!(
        "the edge answered {code}: {}",
        answer["detail"]
            .as_str()
            .or(answer["error"].as_str())
            .unwrap_or("no reason")
    )
}
