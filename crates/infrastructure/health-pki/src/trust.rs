//! Verifying chains against the national root and the CRLs a node holds.

use health_time::Timestamp;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, CertificateRevocationListDer, ServerName, UnixTime};
use webpki::{
    CertRevocationList, EndEntityCert, ExpirationPolicy, KeyUsage, OwnedCertRevocationList,
    RevocationCheckDepth, RevocationOptionsBuilder, UnknownStatusPolicy,
};

use crate::{Error, Result};

/// id-kp-codeSigning (1.3.6.1.5.5.7.3.3), DER-encoded.
const CODE_SIGNING: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x03];
const ALGORITHMS: &[&dyn rustls_pki_types::SignatureVerificationAlgorithm] =
    &[webpki::ring::ECDSA_P256_SHA256];

/// The national root and the CRLs received so far.
///
/// - Every CA in a chain is checked against the CRLs its issuer published; a leaf is not,
///   because facility intermediates publish none and leaves live 397 days.
/// - A CA with no CRL from its issuer yet is accepted, and an out-of-date CRL still counts:
///   nodes can be offline for months, and an old CRL still names what it revoked.
pub struct Trust {
    root: CertificateDer<'static>,
    crls: Vec<CertRevocationList<'static>>,
}

impl Trust {
    /// Trusting the root in `root_pem`.
    pub fn new(root_pem: &str) -> Result<Self> {
        let root = CertificateDer::from_pem_slice(root_pem.as_bytes())
            .map_err(|e| Error::Malformed(format!("root: {e}")))?;
        webpki::anchor_from_trusted_cert(&root)?;
        Ok(Self {
            root,
            crls: Vec::new(),
        })
    }

    /// Adds a CRL received from central. Its signature is checked when a chain uses it, so a
    /// forged CRL makes the chain it names fail rather than pass.
    pub fn add_crl(&mut self, crl_pem: &str) -> Result<()> {
        let der = CertificateRevocationListDer::from_pem_slice(crl_pem.as_bytes())
            .map_err(|e| Error::Malformed(format!("CRL: {e}")))?;
        let crl = OwnedCertRevocationList::from_der(&der)?;
        self.crls.push(CertRevocationList::Owned(crl));
        Ok(())
    }

    /// Verifies a server chain (`chain_pem`, leaf first) for `name`, a DNS name or an IP
    /// address, at `at`.
    pub fn verify_server(&self, chain_pem: &str, name: &str, at: Timestamp) -> Result<()> {
        let chain = certificates(chain_pem)?;
        let (leaf, intermediates) = chain
            .split_first()
            .ok_or_else(|| Error::Malformed("an empty chain".into()))?;
        let leaf = EndEntityCert::try_from(leaf)?;
        self.verify(&leaf, intermediates, KeyUsage::server_auth(), at)?;
        let name = ServerName::try_from(name)
            .map_err(|e| Error::Malformed(format!("server name {name:?}: {e}")))?;
        leaf.verify_is_valid_for_subject_name(&name)?;
        Ok(())
    }

    /// Verifies a publication-signing certificate at `at`, and that it carries code signing.
    pub fn verify_publisher(&self, certificate_pem: &str, at: Timestamp) -> Result<()> {
        let chain = certificates(certificate_pem)?;
        let (certificate, intermediates) = chain
            .split_first()
            .ok_or_else(|| Error::Malformed("no certificate".into()))?;
        let certificate = EndEntityCert::try_from(certificate)?;
        self.verify(
            &certificate,
            intermediates,
            KeyUsage::required(CODE_SIGNING),
            at,
        )
    }

    fn verify(
        &self,
        end: &EndEntityCert<'_>,
        intermediates: &[CertificateDer<'_>],
        usage: KeyUsage,
        at: Timestamp,
    ) -> Result<()> {
        let anchors = [webpki::anchor_from_trusted_cert(&self.root)?];
        let crls: Vec<&CertRevocationList<'_>> = self.crls.iter().collect();
        let revocation = RevocationOptionsBuilder::new(&crls).ok().map(|builder| {
            builder
                .with_depth(RevocationCheckDepth::Chain)
                .with_status_policy(UnknownStatusPolicy::Allow)
                .with_expiration_policy(ExpirationPolicy::Ignore)
                .build()
        });
        let time = UnixTime::since_unix_epoch(std::time::Duration::from_secs(
            u64::try_from(at.as_second())
                .map_err(|_| Error::Malformed("a time before 1970".into()))?,
        ));
        end.verify_for_usage(
            ALGORITHMS,
            &anchors,
            intermediates,
            time,
            usage,
            revocation,
            None,
        )?;
        Ok(())
    }
}

fn certificates(pem: &str) -> Result<Vec<CertificateDer<'static>>> {
    CertificateDer::pem_slice_iter(pem.as_bytes())
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| Error::Malformed(format!("certificate: {e}")))
}
