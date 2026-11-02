//! Reading a certificate back: what an edge needs to decide whether its leaf must be renewed.

use std::net::IpAddr;

use health_time::Timestamp;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use x509_parser::extensions::GeneralName;

use crate::{Error, Result};

/// A certificate's validity and the names it is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspected {
    /// When it starts to be valid (backdated by [`BACKDATE_SECONDS`](crate::BACKDATE_SECONDS)).
    pub not_before: Timestamp,
    /// When it expires.
    pub not_after: Timestamp,
    /// Its DNS subject alternative names, in order.
    pub dns_names: Vec<String>,
    /// Its IP address subject alternative names, in order.
    pub addresses: Vec<IpAddr>,
    /// The SHA-256 of its SubjectPublicKeyInfo, as `sha256/<base64>`: what a client pins
    /// (lan-security.md: Tauri pins the facility intermediate's SPKI).
    pub spki_sha256: String,
}

/// Reads the first certificate in `pem`.
pub fn inspect(pem: &str) -> Result<Inspected> {
    let certificate = CertificateDer::from_pem_slice(pem.as_bytes())
        .map_err(|e| Error::Malformed(format!("certificate: {e}")))?;
    let (_, x509) = x509_parser::parse_x509_certificate(&certificate)
        .map_err(|e| Error::Malformed(format!("certificate: {e}")))?;
    let at =
        |t: i64| Timestamp::from_second(t).map_err(|e| Error::Malformed(format!("validity: {e}")));
    let (mut dns_names, mut addresses) = (Vec::new(), Vec::new());
    if let Ok(Some(san)) = x509.subject_alternative_name() {
        for name in &san.value.general_names {
            match name {
                GeneralName::DNSName(name) => dns_names.push((*name).to_owned()),
                GeneralName::IPAddress(bytes) => match bytes.len() {
                    4 => addresses.push(IpAddr::from(
                        <[u8; 4]>::try_from(*bytes).unwrap_or_default(),
                    )),
                    16 => addresses.push(IpAddr::from(
                        <[u8; 16]>::try_from(*bytes).unwrap_or_default(),
                    )),
                    _ => return Err(Error::Malformed("an IP address SAN of odd length".into())),
                },
                _ => {}
            }
        }
    }
    let spki_sha256 = {
        use base64::Engine as _;
        use sha2::Digest as _;
        let digest = sha2::Sha256::digest(x509.public_key().raw);
        format!(
            "sha256/{}",
            base64::engine::general_purpose::STANDARD.encode(digest)
        )
    };
    Ok(Inspected {
        spki_sha256,
        not_before: at(x509.validity().not_before.timestamp())?,
        not_after: at(x509.validity().not_after.timestamp())?,
        dns_names,
        addresses,
    })
}
