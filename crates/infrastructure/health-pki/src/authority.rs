//! CAs and what each tier may issue.

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use health_time::Timestamp;
use rand_core::{OsRng, RngCore};
use rcgen::{
    BasicConstraints, CertificateParams, CertificateRevocationListParams,
    CertificateSigningRequestParams, CidrSubnet, DnType, ExtendedKeyUsagePurpose, GeneralSubtree,
    IsCa, Issuer, KeyIdMethod, KeyPair, KeyUsagePurpose, NameConstraints, PKCS_ECDSA_P256_SHA256,
    PublicKeyData, RevokedCertParams, SanType, SerialNumber,
};
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use time::{Duration, OffsetDateTime};
use x509_parser::extensions::GeneralName;

use crate::revocation::Revoked;
use crate::{
    BACKDATE_SECONDS, CRL_DAYS, Error, FACILITY_YEARS, ISSUING_CA_YEARS, LEAF_DAYS,
    PUBLICATION_DAYS, ROOT_YEARS, Result,
};

/// The RFC 1918 and ULA ranges: the only addresses an edge certificate may name.
const PRIVATE: [(IpAddr, u8); 4] = [
    (IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 0)), 8),
    (IpAddr::V4(std::net::Ipv4Addr::new(172, 16, 0, 0)), 12),
    (IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 0, 0)), 16),
    (
        IpAddr::V6(std::net::Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0)),
        7,
    ),
];

/// A CA's place in the hierarchy, read from its path length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The national root (path length 2).
    Root,
    /// The facility issuing CA (path length 1).
    IssuingCa,
    /// A facility intermediate (path length 0).
    Facility,
}

/// A CA with its key at hand.
pub struct Authority {
    certificate: CertificateDer<'static>,
    key: KeyPair,
    tier: Tier,
    /// The DNS subtrees its name constraints permit; empty for the unconstrained root.
    permitted: Vec<String>,
    not_after: OffsetDateTime,
}

/// The facility issuing CA to issue.
#[derive(Debug, Clone)]
pub struct IssuingCa {
    /// Its common name.
    pub common_name: String,
    /// The country's configured facility DNS suffixes; `.local` is always permitted.
    pub dns_suffixes: Vec<String>,
}

/// The facility whose intermediate to issue.
#[derive(Debug, Clone)]
pub struct Facility {
    /// The edge's node ID: the intermediate permits `<node-id>.local`.
    pub node_id: String,
    /// The facility's configured DNS names, each under one of the issuing CA's suffixes.
    pub dns_names: Vec<String>,
}

/// A certificate just issued.
#[derive(Debug, Clone)]
pub struct Issued {
    /// The certificate.
    pub certificate: CertificateDer<'static>,
    /// Its serial number, to revoke it by.
    pub serial: Serial,
}

impl Issued {
    /// The certificate as PEM.
    pub fn pem(&self) -> String {
        pem(&self.certificate)
    }
}

/// A certificate serial number, written as lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Serial(Vec<u8>);

impl Serial {
    /// A fresh 128-bit serial, positive and with no leading zero byte.
    fn random() -> Self {
        let mut bytes = [0; 16];
        OsRng.fill_bytes(&mut bytes);
        bytes[0] = (bytes[0] & 0x7f) | 0x40;
        Self(bytes.to_vec())
    }

    pub(crate) fn number(&self) -> SerialNumber {
        SerialNumber::from_slice(&self.0)
    }
}

impl fmt::Display for Serial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl FromStr for Serial {
    type Err = Error;

    fn from_str(hex: &str) -> Result<Self> {
        let bad = || Error::Malformed(format!("serial {hex:?}"));
        if hex.is_empty() || !hex.len().is_multiple_of(2) || hex.len() > 40 {
            return Err(bad());
        }
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(hex.get(i..i + 2).ok_or_else(bad)?, 16).map_err(|_| bad()))
            .collect::<Result<_>>()
            .map(Self)
    }
}

impl serde::Serialize for Serial {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Serial {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let hex = String::deserialize(d)?;
        hex.parse().map_err(serde::de::Error::custom)
    }
}

/// A fresh ECDSA P-256 key.
pub fn new_key() -> Result<KeyPair> {
    Ok(KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?)
}

/// A certificate request for `key`, as PEM: what a holder sends its issuer.
pub fn request(key: &KeyPair, common_name: &str) -> Result<String> {
    let mut params = CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, common_name);
    Ok(params.serialize_request(key)?.pem()?)
}

impl Authority {
    /// A new national root, self-signed at `at`: run at the root ceremony.
    pub fn root(common_name: &str, at: Timestamp) -> Result<Self> {
        let key = new_key()?;
        let from = instant(at)?;
        let mut params = ca_params(common_name, 2, from, years(from, ROOT_YEARS)?);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let certificate = params.self_signed(&key)?;
        Self::new(certificate.der().clone(), key)
    }

    /// A CA from its certificate and key, as PEM.
    pub fn from_pem(certificate: &str, key: &str) -> Result<Self> {
        let certificate = CertificateDer::from_pem_slice(certificate.as_bytes())
            .map_err(|e| Error::Malformed(format!("certificate: {e}")))?;
        Self::new(certificate, KeyPair::from_pem(key)?)
    }

    fn new(certificate: CertificateDer<'static>, key: KeyPair) -> Result<Self> {
        let (_, x509) = x509_parser::parse_x509_certificate(&certificate)
            .map_err(|e| Error::Malformed(format!("certificate: {e}")))?;
        let tier = match x509.basic_constraints().ok().flatten().map(|e| e.value) {
            Some(bc) if bc.ca => match bc.path_len_constraint {
                Some(2) => Tier::Root,
                Some(1) => Tier::IssuingCa,
                Some(0) => Tier::Facility,
                other => {
                    return Err(Error::Refused(format!(
                        "a CA with path length {other:?} is not part of this PKI"
                    )));
                }
            },
            _ => return Err(Error::Refused("not a CA certificate".into())),
        };
        if x509.public_key().subject_public_key.data.as_ref() != key.public_key_raw() {
            return Err(Error::Refused(
                "the key does not belong to the certificate".into(),
            ));
        }
        let permitted = x509
            .name_constraints()
            .ok()
            .flatten()
            .and_then(|e| e.value.permitted_subtrees.as_ref())
            .map(|subtrees| {
                subtrees
                    .iter()
                    .filter_map(|s| match s.base {
                        GeneralName::DNSName(name) => Some(name.to_owned()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let not_after = OffsetDateTime::from_unix_timestamp(x509.validity().not_after.timestamp())
            .map_err(|e| Error::Malformed(format!("validity: {e}")))?;
        drop(x509);
        Ok(Self {
            certificate,
            key,
            tier,
            permitted,
            not_after,
        })
    }

    /// Its tier.
    pub fn tier(&self) -> Tier {
        self.tier
    }

    /// When its certificate expires.
    pub fn expires(&self) -> Timestamp {
        Timestamp::from_second(self.not_after.unix_timestamp()).unwrap_or(Timestamp::MAX)
    }

    /// Its certificate.
    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.certificate
    }

    /// Its certificate as PEM.
    pub fn certificate_pem(&self) -> String {
        pem(&self.certificate)
    }

    /// Its private key as PKCS#8 PEM, for the ceremony's encrypted media only.
    pub fn key_pem(&self) -> String {
        self.key.serialize_pem()
    }

    /// The root issues the facility issuing CA from its request (ADR 0058).
    pub fn issue_issuing_ca(&self, request: &str, ca: &IssuingCa, at: Timestamp) -> Result<Issued> {
        self.require(Tier::Root, "an issuing CA")?;
        let mut permitted = vec![GeneralSubtree::DnsName("local".into())];
        for suffix in &ca.dns_suffixes {
            dns_name(suffix)?;
            if suffix == "local" {
                continue;
            }
            permitted.push(GeneralSubtree::DnsName(suffix.clone()));
        }
        let from = instant(at)?;
        let mut params = ca_params(&ca.common_name, 1, from, years(from, ISSUING_CA_YEARS)?);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.name_constraints = Some(constraints(permitted));
        self.sign(params, request)
    }

    /// The root issues a publication-signing certificate from its request: code signing only,
    /// never a CA (configuration.md § Signing).
    pub fn issue_publication(
        &self,
        request: &str,
        common_name: &str,
        at: Timestamp,
    ) -> Result<Issued> {
        self.require(Tier::Root, "a publication-signing certificate")?;
        let from = instant(at)?;
        let mut params = leaf_params(common_name, from, from + Duration::days(PUBLICATION_DAYS));
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::CodeSigning];
        self.sign(params, request)
    }

    /// The issuing CA issues a facility's intermediate from the edge's request at provisioning.
    pub fn issue_facility(
        &self,
        request: &str,
        facility: &Facility,
        at: Timestamp,
    ) -> Result<Issued> {
        self.require(Tier::IssuingCa, "a facility intermediate")?;
        dns_label(&facility.node_id)?;
        let mut names = vec![format!("{}.local", facility.node_id)];
        for name in &facility.dns_names {
            dns_name(name)?;
            names.push(name.clone());
        }
        for name in &names {
            self.permits(name)?;
        }
        let from = instant(at)?;
        let mut params = ca_params(
            &format!("{} facility intermediate", facility.node_id),
            0,
            from,
            years(from, FACILITY_YEARS)?,
        );
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.name_constraints = Some(constraints(
            names.into_iter().map(GeneralSubtree::DnsName).collect(),
        ));
        self.sign(params, request)
    }

    /// A facility intermediate issues its edge's server leaf for `key`, naming its DNS names and
    /// private addresses (lan-security.md § Certificate hierarchy).
    pub fn issue_server(
        &self,
        key: &KeyPair,
        dns_names: &[String],
        addresses: &[IpAddr],
        at: Timestamp,
    ) -> Result<Issued> {
        self.require(Tier::Facility, "a server certificate")?;
        let first = dns_names
            .first()
            .ok_or_else(|| Error::Refused("a server certificate needs a DNS name".into()))?;
        let mut sans = Vec::new();
        for name in dns_names {
            dns_name(name)?;
            self.permits(name)?;
            sans.push(SanType::DnsName(name.as_str().try_into()?));
        }
        for address in addresses {
            if !private(*address) {
                return Err(Error::Refused(format!(
                    "{address} is not a private address"
                )));
            }
            sans.push(SanType::IpAddress(*address));
        }
        let from = instant(at)?;
        let mut params = leaf_params(first, from, from + Duration::days(LEAF_DAYS));
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.subject_alt_names = sans;
        self.sign_key(params, key)
    }

    /// A CRL listing `revoked`, numbered `number`, current for [`CRL_DAYS`] from `at`.
    pub(crate) fn crl(&self, revoked: &[Revoked], number: u64, at: Timestamp) -> Result<String> {
        if self.tier == Tier::Facility {
            return Err(Error::Refused(
                "a facility intermediate does not publish a CRL".into(),
            ));
        }
        let this_update = instant(at)?;
        let params = CertificateRevocationListParams {
            this_update,
            next_update: this_update + Duration::days(CRL_DAYS),
            crl_number: SerialNumber::from_slice(&number.to_be_bytes()),
            issuing_distribution_point: None,
            revoked_certs: revoked
                .iter()
                .map(|r| {
                    Ok(RevokedCertParams {
                        serial_number: r.serial.number(),
                        revocation_time: instant(r.at)?,
                        reason_code: Some(r.reason.code()),
                        invalidity_date: None,
                    })
                })
                .collect::<Result<_>>()?,
            key_identifier_method: KeyIdMethod::Sha256,
        };
        Ok(params.signed_by(&self.issuer()?)?.pem()?)
    }

    fn require(&self, tier: Tier, what: &str) -> Result<()> {
        if self.tier == tier {
            Ok(())
        } else {
            Err(Error::Refused(format!(
                "a {:?} may not issue {what}",
                self.tier
            )))
        }
    }

    /// Refuses `name` unless this CA's constraints permit it.
    fn permits(&self, name: &str) -> Result<()> {
        let within = |subtree: &String| {
            name == subtree
                || name
                    .strip_suffix(subtree.as_str())
                    .is_some_and(|rest| rest.ends_with('.'))
        };
        if self.permitted.iter().any(within) {
            Ok(())
        } else {
            Err(Error::Refused(format!(
                "{name} is outside the issuer's names ({})",
                self.permitted.join(", ")
            )))
        }
    }

    fn issuer(&self) -> Result<Issuer<'_, &KeyPair>> {
        Ok(Issuer::from_ca_cert_der(&self.certificate, &self.key)?)
    }

    /// Signs `params` for the public key in `request`, ignoring everything else it asks for.
    fn sign(&self, params: CertificateParams, request: &str) -> Result<Issued> {
        let request = CertificateSigningRequestParams::from_pem(request)?;
        if request.public_key.algorithm() != &PKCS_ECDSA_P256_SHA256 {
            return Err(Error::Refused(
                "the request's key is not ECDSA P-256".into(),
            ));
        }
        self.sign_key(params, &request.public_key)
    }

    fn sign_key(&self, mut params: CertificateParams, key: &impl PublicKeyData) -> Result<Issued> {
        if params.not_after > self.not_after {
            return Err(Error::Refused(
                "the certificate would outlive its issuer; a successor issuer is due".into(),
            ));
        }
        let serial = Serial::random();
        params.serial_number = Some(serial.number());
        params.use_authority_key_identifier_extension = true;
        let certificate = params.signed_by(key, &self.issuer()?)?;
        Ok(Issued {
            certificate: certificate.der().clone(),
            serial,
        })
    }
}

fn ca_params(
    common_name: &str,
    path_length: u8,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> CertificateParams {
    let mut params = CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, common_name);
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(path_length));
    params.not_before = from - Duration::seconds(BACKDATE_SECONDS);
    params.not_after = to;
    params.serial_number = Some(Serial::random().number());
    params
}

fn leaf_params(common_name: &str, from: OffsetDateTime, to: OffsetDateTime) -> CertificateParams {
    let mut params = ca_params(common_name, 0, from, to);
    params.is_ca = IsCa::ExplicitNoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params
}

fn constraints(mut permitted: Vec<GeneralSubtree>) -> NameConstraints {
    permitted.extend(PRIVATE.iter().map(|(addr, prefix)| {
        GeneralSubtree::IpAddress(CidrSubnet::from_addr_prefix(*addr, *prefix))
    }));
    NameConstraints {
        permitted_subtrees: permitted,
        excluded_subtrees: Vec::new(),
    }
}

fn private(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => ip.is_private(),
        IpAddr::V6(ip) => ip.segments()[0] & 0xfe00 == 0xfc00,
    }
}

/// Refuses anything but a lowercase DNS name with no wildcard.
fn dns_name(name: &str) -> Result<()> {
    if name.len() > 253 || name.split('.').any(|label| dns_label(label).is_err()) {
        return Err(Error::Refused(format!(
            "{name:?} is not a lowercase DNS name"
        )));
    }
    Ok(())
}

fn dns_label(label: &str) -> Result<()> {
    let valid = !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if valid {
        Ok(())
    } else {
        Err(Error::Refused(format!(
            "{label:?} is not a lowercase DNS label"
        )))
    }
}

pub(crate) fn instant(at: Timestamp) -> Result<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(at.as_second())
        .map_err(|e| Error::Malformed(format!("time: {e}")))
}

/// `n` calendar years after `from`; 29 February becomes 28 February.
fn years(from: OffsetDateTime, n: i32) -> Result<OffsetDateTime> {
    let year = from.year() + n;
    from.replace_year(year)
        .or_else(|_| (from - Duration::days(1)).replace_year(year))
        .map_err(|e| Error::Malformed(format!("time: {e}")))
}

/// `der` as a PEM `CERTIFICATE` block.
pub(crate) fn pem(der: &[u8]) -> String {
    use base64::Engine;
    let body = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    for line in body.as_bytes().chunks(64) {
        out.push_str(&String::from_utf8_lossy(line));
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}
