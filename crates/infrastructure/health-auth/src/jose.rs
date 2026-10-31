//! Compact JWS with ES256 (RFC 7515, 7518), JWK thumbprints (RFC 7638) and DPoP proofs
//! (RFC 9449), over RustCrypto's `p256`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::elliptic_curve::sec1::{EncodedPoint, FromEncodedPoint};
use p256::{AffinePoint, NistP256};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// A JOSE failure. Messages never quote token contents.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JoseError {
    /// Not three base64url parts of JSON.
    #[error("malformed JWS")]
    Malformed,
    /// Not ES256, or the wrong `typ`.
    #[error("unsupported JWS header")]
    Header,
    /// The signature does not verify.
    #[error("bad signature")]
    Signature,
    /// The key is not a public P-256 JWK.
    #[error("unsupported key")]
    Key,
}

/// Base64url without padding.
pub fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64(text: &str) -> Result<Vec<u8>, JoseError> {
    URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| JoseError::Malformed)
}

/// `b64(sha256(text))`: DPoP's `ath` and PKCE's S256 challenge.
pub fn sha256_b64(text: &str) -> String {
    b64(&Sha256::digest(text.as_bytes()))
}

/// A public P-256 key as a JWK.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcJwk {
    /// `EC`.
    pub kty: String,
    /// `P-256`.
    pub crv: String,
    /// The x coordinate, base64url.
    pub x: String,
    /// The y coordinate, base64url.
    pub y: String,
}

impl EcJwk {
    /// The JWK of `key`.
    pub fn of(key: &VerifyingKey) -> Self {
        let point = key.to_encoded_point(false);
        Self {
            kty: "EC".into(),
            crv: "P-256".into(),
            x: b64(point.x().map(|x| x.as_slice()).unwrap_or_default()),
            y: b64(point.y().map(|y| y.as_slice()).unwrap_or_default()),
        }
    }

    /// The verifying key this JWK describes.
    pub fn key(&self) -> Result<VerifyingKey, JoseError> {
        if self.kty != "EC" || self.crv != "P-256" {
            return Err(JoseError::Key);
        }
        let (x, y) = (unb64(&self.x)?, unb64(&self.y)?);
        if x.len() != 32 || y.len() != 32 {
            return Err(JoseError::Key);
        }
        let point = EncodedPoint::<NistP256>::from_affine_coordinates(
            x.as_slice().into(),
            y.as_slice().into(),
            false,
        );
        let affine = Option::<AffinePoint>::from(AffinePoint::from_encoded_point(&point))
            .ok_or(JoseError::Key)?;
        VerifyingKey::from_affine(affine).map_err(|_| JoseError::Key)
    }

    /// The RFC 7638 thumbprint: DPoP's `jkt` and the signing key's `kid`.
    pub fn thumbprint(&self) -> String {
        // The required members, in lexicographic order, without whitespace.
        let canonical = format!(
            r#"{{"crv":"{}","kty":"{}","x":"{}","y":"{}"}}"#,
            self.crv, self.kty, self.x, self.y
        );
        b64(&Sha256::digest(canonical.as_bytes()))
    }
}

/// The server's token-signing key.
#[derive(Clone)]
pub struct TokenSigner {
    key: SigningKey,
    kid: String,
}

impl std::fmt::Debug for TokenSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenSigner")
            .field("kid", &self.kid)
            .finish_non_exhaustive()
    }
}

impl TokenSigner {
    /// A fresh random key (tests and development; production keys come from the keystore).
    pub fn generate() -> Self {
        Self::new(SigningKey::random(&mut rand_core::OsRng))
    }

    /// A signer over `key`; its `kid` is the key's thumbprint.
    pub fn new(key: SigningKey) -> Self {
        let kid = EcJwk::of(key.verifying_key()).thumbprint();
        Self { key, kid }
    }

    /// The key id.
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The public key set, for `/jwks`.
    pub fn jwks(&self) -> Value {
        let jwk = EcJwk::of(self.key.verifying_key());
        json!({ "keys": [{ "kty": jwk.kty, "crv": jwk.crv, "x": jwk.x, "y": jwk.y, "kid": self.kid, "use": "sig", "alg": "ES256" }] })
    }

    /// Signs `claims` as a JWT.
    pub fn sign(&self, claims: &Value) -> String {
        let header = json!({ "alg": "ES256", "typ": "at+jwt", "kid": self.kid });
        sign(&self.key, &header, claims)
    }

    /// Signs `claims` as an OpenID Connect ID token (`typ` JWT).
    pub fn sign_id_token(&self, claims: &Value) -> String {
        let header = json!({ "alg": "ES256", "typ": "JWT", "kid": self.kid });
        sign(&self.key, &header, claims)
    }

    /// Verifies a token this signer issued and returns its claims.
    pub fn verify(&self, token: &str) -> Result<Value, JoseError> {
        let (header, claims, input, signature) = split(token)?;
        if header["alg"] != "ES256" || header["kid"] != self.kid.as_str() {
            return Err(JoseError::Header);
        }
        self.key
            .verifying_key()
            .verify(input.as_bytes(), &signature)
            .map_err(|_| JoseError::Signature)?;
        Ok(claims)
    }
}

/// Signs a compact JWS.
pub fn sign(key: &SigningKey, header: &Value, claims: &Value) -> String {
    let input = format!(
        "{}.{}",
        b64(header.to_string().as_bytes()),
        b64(claims.to_string().as_bytes())
    );
    let signature: Signature = key.sign(input.as_bytes());
    format!("{input}.{}", b64(&signature.to_bytes()))
}

/// The header, claims, signing input and signature of a compact JWS.
pub(crate) fn split(token: &str) -> Result<(Value, Value, &str, Signature), JoseError> {
    let mut parts = token.split('.');
    let (Some(header), Some(claims), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(JoseError::Malformed);
    };
    let input = &token[..header.len() + 1 + claims.len()];
    let parse = |part: &str| -> Result<Value, JoseError> {
        serde_json::from_slice(&unb64(part)?).map_err(|_| JoseError::Malformed)
    };
    let signature = Signature::from_slice(&unb64(signature)?).map_err(|_| JoseError::Signature)?;
    Ok((parse(header)?, parse(claims)?, input, signature))
}

/// A verified DPoP proof (RFC 9449 § 4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpopProof {
    /// The thumbprint of the proof's key: what a bound token's `cnf.jkt` must equal.
    pub jkt: String,
    /// The proof's unique id, for replay detection.
    pub jti: String,
    /// When the proof says it was made (seconds since the epoch).
    pub iat: i64,
    /// The access-token hash, when the proof accompanies a token.
    pub ath: Option<String>,
}

/// Verifies a DPoP proof's signature, type, key and its `htm` and `htu` against the request.
/// Freshness and replay are the caller's (they need a clock and a jti cache).
pub fn verify_dpop(proof: &str, method: &str, uri: &str) -> Result<DpopProof, JoseError> {
    let (header, claims, input, signature) = split(proof)?;
    if header["typ"] != "dpop+jwt" || header["alg"] != "ES256" {
        return Err(JoseError::Header);
    }
    // A private key in the header is refused (RFC 9449 § 4.3, check 7).
    if header["jwk"].get("d").is_some() {
        return Err(JoseError::Key);
    }
    let jwk: EcJwk = serde_json::from_value(header["jwk"].clone()).map_err(|_| JoseError::Key)?;
    jwk.key()?
        .verify(input.as_bytes(), &signature)
        .map_err(|_| JoseError::Signature)?;
    let without_query = |u: &str| u.split(['?', '#']).next().unwrap_or_default().to_owned();
    let (Some(htm), Some(htu), Some(jti), Some(iat)) = (
        claims["htm"].as_str(),
        claims["htu"].as_str(),
        claims["jti"].as_str(),
        claims["iat"].as_i64(),
    ) else {
        return Err(JoseError::Malformed);
    };
    if htm != method || without_query(htu) != without_query(uri) || jti.is_empty() {
        return Err(JoseError::Header);
    }
    Ok(DpopProof {
        jkt: jwk.thumbprint(),
        jti: jti.to_owned(),
        iat,
        ath: claims["ath"].as_str().map(str::to_owned),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A client's DPoP proof, as `dpopAuth` in the web client makes it.
    pub(crate) fn proof(
        key: &SigningKey,
        method: &str,
        uri: &str,
        iat: i64,
        token: Option<&str>,
    ) -> String {
        let jwk = EcJwk::of(key.verifying_key());
        let mut claims = json!({ "jti": uuid::Uuid::now_v7().to_string(), "htm": method, "htu": uri, "iat": iat });
        if let Some(token) = token {
            claims["ath"] = json!(sha256_b64(token));
        }
        sign(
            key,
            &json!({ "typ": "dpop+jwt", "alg": "ES256", "jwk": jwk }),
            &claims,
        )
    }

    #[test]
    fn tokens_verify_and_tampering_fails() {
        let signer = TokenSigner::generate();
        let token = signer.sign(&json!({ "sub": "a" }));
        assert_eq!(
            signer.verify(&token).map(|c| c["sub"].clone()),
            Ok(json!("a"))
        );
        let mut parts: Vec<&str> = token.split('.').collect();
        let forged = b64(br#"{"sub":"b"}"#);
        parts[1] = &forged;
        assert_eq!(signer.verify(&parts.join(".")), Err(JoseError::Signature));
        assert!(TokenSigner::generate().verify(&token).is_err());
    }

    #[test]
    fn the_thumbprint_is_rfc_7638() {
        // RFC 7638 has no P-256 example; this checks the canonical form against a fixed key.
        let jwk = EcJwk {
            kty: "EC".into(),
            crv: "P-256".into(),
            x: "a".into(),
            y: "b".into(),
        };
        assert_eq!(
            jwk.thumbprint(),
            b64(&Sha256::digest(
                br#"{"crv":"P-256","kty":"EC","x":"a","y":"b"}"#
            ))
        );
    }

    #[test]
    fn dpop_proofs_bind_method_uri_and_key() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let uri = "https://edge.example/token";
        let verified = verify_dpop(
            &proof(&key, "POST", uri, 1, None),
            "POST",
            "https://edge.example/token?x=1",
        )
        .expect("valid");
        assert_eq!(verified.jkt, EcJwk::of(key.verifying_key()).thumbprint());
        assert_eq!(
            verify_dpop(&proof(&key, "GET", uri, 1, None), "POST", uri),
            Err(JoseError::Header)
        );
        assert_eq!(
            verify_dpop(
                &proof(&key, "POST", "https://other/token", 1, None),
                "POST",
                uri
            ),
            Err(JoseError::Header)
        );
        let bearer = TokenSigner::generate().sign(&json!({}));
        assert!(verify_dpop(&bearer, "POST", uri).is_err());
    }
}
