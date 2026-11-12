//! Node-signed sync requests (ADR 0062): every `/sync/v1` request carries a compact ES256 JWS
//! from the node's facility-intermediate key, bound to the request's method, path and body,
//! and central checks it against the node registry.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use health_identity::{Node, NodeId, NodeStatus};
use health_time::Timestamp;
use p256::ecdsa::signature::{Signer as _, Verifier as _};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use serde_json::{Value, json};
use sha2::Digest as _;

use crate::{Error, NodeRegistry};

/// The JWS `typ` of a sync request.
pub const SYNC_TYP: &str = "health-sync+jwt";

/// How far a sync request's signing time may be from central's clock, in seconds.
pub const SYNC_SKEW_SECS: i64 = 300;

/// The `Authorization` scheme a sync request's JWS is sent with.
pub const SYNC_SCHEME: &str = "Health-Node";

fn body_hash(body: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(body))
}

/// Signs one sync request from `node`: `method` and `path` as sent, `body` as sent, at `at`,
/// with the facility intermediate's `key`. Good for that request only.
pub fn sync_jws(
    node: &str,
    method: &str,
    path: &str,
    body: &[u8],
    key: &SigningKey,
    at: Timestamp,
) -> String {
    let header = URL_SAFE_NO_PAD.encode(json!({ "alg": "ES256", "typ": SYNC_TYP }).to_string());
    let payload = URL_SAFE_NO_PAD.encode(
        json!({
            "node": node,
            "iat": at.as_second(),
            "htm": method,
            "htu": path,
            "bh": body_hash(body),
        })
        .to_string(),
    );
    let signature: Signature = key.sign(format!("{header}.{payload}").as_bytes());
    format!(
        "{header}.{payload}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    )
}

/// A DPoP proof (RFC 9449) for one request by an enrolled LAN device: `method` to `htu` (the
/// origin and path as sent) at `at`, signed with the device's `key`. A desktop under an edge
/// syncs with these rather than [`sync_jws`]: it is an enrolled device there, not a provisioned
/// node (ADR 0062). The proof does not cover the body; the LAN's TLS does.
pub fn dpop_proof(method: &str, htu: &str, key: &SigningKey, at: Timestamp) -> String {
    proof(method, htu, key, at, None)
}

/// [`dpop_proof`] for a request that carries `access_token`, a DPoP-bound token: the proof's
/// `ath` is the token's hash (RFC 9449 § 4.2).
pub fn dpop_proof_with_token(
    method: &str,
    htu: &str,
    key: &SigningKey,
    at: Timestamp,
    access_token: &str,
) -> String {
    proof(method, htu, key, at, Some(access_token))
}

fn proof(
    method: &str,
    htu: &str,
    key: &SigningKey,
    at: Timestamp,
    access_token: Option<&str>,
) -> String {
    let point = key.verifying_key().to_encoded_point(false);
    let coordinate = |c: Option<&p256::FieldBytes>| {
        URL_SAFE_NO_PAD.encode(c.map(|c| c.as_slice()).unwrap_or_default())
    };
    let header = URL_SAFE_NO_PAD.encode(
        json!({
            "typ": "dpop+jwt",
            "alg": "ES256",
            "jwk": { "kty": "EC", "crv": "P-256", "x": coordinate(point.x()), "y": coordinate(point.y()) },
        })
        .to_string(),
    );
    let mut claims = json!({
        "jti": uuid::Uuid::now_v7().to_string(),
        "htm": method,
        "htu": htu,
        "iat": at.as_second(),
    });
    if let Some(token) = access_token {
        claims["ath"] = json!(URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(token.as_bytes())));
    }
    let payload = URL_SAFE_NO_PAD.encode(claims.to_string());
    let signature: Signature = key.sign(format!("{header}.{payload}").as_bytes());
    format!(
        "{header}.{payload}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    )
}

impl NodeRegistry {
    /// The active node that signed this sync request, after checking the JWS against the
    /// node's key on record, the signing time against central's clock, and the method, path and
    /// body against the request (ADR 0062).
    pub async fn authenticate_sync(
        &self,
        jws: &str,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Result<Node, Error> {
        let malformed = |why: &str| Error::Malformed(why.to_owned());
        let unauthorized = |why: String| Error::Unauthorized(why);
        let mut parts = jws.trim().split('.');
        let (Some(header), Some(payload), Some(signature), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(malformed("not a compact JWS"));
        };
        let decode = |part: &str| -> Result<Value, Error> {
            URL_SAFE_NO_PAD
                .decode(part)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .ok_or_else(|| malformed("unreadable JWS"))
        };
        let (head, claims) = (decode(header)?, decode(payload)?);
        if head["alg"] != "ES256" || head["typ"] != SYNC_TYP || head.get("crit").is_some() {
            return Err(malformed("not a sync request token"));
        }
        let node_id = claims["node"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .map(NodeId)
            .ok_or_else(|| malformed("no node"))?;
        let signed_at = claims["iat"]
            .as_i64()
            .ok_or_else(|| malformed("no signing time"))?;

        let mut tx = self.begin().await?;
        let node = self
            .node(&mut tx, &node_id.to_string())
            .await?
            .filter(|node| node.status == NodeStatus::Active)
            .ok_or_else(|| unauthorized("no active node by that id".into()))?;
        let key = URL_SAFE_NO_PAD
            .decode(&node.intermediate_key)
            .ok()
            .and_then(|sec1| VerifyingKey::from_sec1_bytes(&sec1).ok())
            .ok_or_else(|| unauthorized("the node has no key on record".into()))?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .ok()
            .and_then(|bytes| Signature::from_slice(&bytes).ok())
            .ok_or_else(|| unauthorized("unreadable signature".into()))?;
        key.verify(format!("{header}.{payload}").as_bytes(), &signature)
            .map_err(|_| unauthorized("the signature is not the node's".into()))?;

        let skew = self.clock.now().as_second() - signed_at;
        if skew.abs() > SYNC_SKEW_SECS {
            return Err(unauthorized(format!(
                "the request was signed {skew} s from central's clock; at most {SYNC_SKEW_SECS} s \
                 is allowed, so check the node's clock"
            )));
        }
        if claims["htm"].as_str() != Some(method) || claims["htu"].as_str() != Some(path) {
            return Err(unauthorized(
                "the token was signed for another request".into(),
            ));
        }
        if claims["bh"].as_str() != Some(body_hash(body).as_str()) {
            return Err(unauthorized("the token was signed for another body".into()));
        }
        Ok(node)
    }
}
