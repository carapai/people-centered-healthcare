//! Central's node registry and edge provisioning (identifiers.md; lan-security.md; ADRs 0048,
//! 0058; #268).
//!
//! - [`NodeRegistry::issue_code`] issues a one-time provisioning code for a facility, optionally
//!   with an encryption exemption and its reason. Only the code's sha-256 is stored; the text is
//!   shown to the operator once.
//! - [`NodeRegistry::redeem`] is what an edge does with it, through [`router`]'s
//!   `POST /provisioning/nodes`: in one transaction, the code is checked
//!   ([`health_identity::ProvisioningCode::redeemable`]), the node gets its id and next short
//!   code, the [`Issuer`] issues its facility intermediate for `<node-id>.local` and the code's
//!   DNS names, and the node, the redeemed code and an AuditEvent are written. Nothing commits
//!   if issuance fails, so the code stays redeemable. Unique indexes refuse a concurrent second
//!   redemption at commit.
//! - **Exemptions** (ADR 0048): an exempt node is audited as a warning, raises an
//!   `encryption-exempt` alert when provisioned, and stands in [`NodeRegistry::exempt`] for
//!   central to keep alerting on.
//! - **Recovery keys** (ADR 0048): [`NodeRegistry::escrow`] takes an edge's disk recovery keys in
//!   a JWS ([`recovery_jws`]) signed with its facility intermediate's key, which central recorded
//!   at provisioning. It wraps each key through the [`KeyWrap`] port, never storing it in the
//!   clear, and refuses an escrow no newer than the key it replaces.
//!   [`NodeRegistry::read_recovery`] is the audited phone read-out, with who read it and how
//!   they verified the caller. A key read out is used, and stays in
//!   [`NodeRegistry::used_recovery_keys`] for central to alert on until the edge escrows its
//!   replacement.

mod kms;
mod routes;

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use health_identity::{
    CodeRefused, Encryption, EscrowRefused, Node, NodeId, NodeKind, NodeStatus, ProvisioningCode,
    ProvisioningPolicy, RecoveryKey, RecoveryVolume, Retrieval,
};
use health_persistence::{Store, Transaction};
use health_pki::{Authority, Facility};
use health_surrealdb::{SurrealStore, SurrealTx};
use health_time::{Clock, Timestamp};
use p256::ecdsa::signature::{Signer as _, Verifier as _};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use rand_core::RngCore;
use serde_json::{Value, json};
use sha2::Digest as _;
use uuid::Uuid;

pub use kms::{KeyWrap, LocalKeyWrap};
pub use routes::{recovery_router, router};

/// What issues facility intermediates: central's issuing CA, whose key is held in the HSM or KMS
/// (security.md § Keys), so issuance is a port.
pub trait Issuer: Send + Sync {
    /// Issues the facility intermediate for `request` (a PKCS#10 PEM) at `at`; returns it and its
    /// serial.
    fn issue(
        &self,
        request: &str,
        facility: &Facility,
        at: Timestamp,
    ) -> Result<(String, String), String>;
    /// The issuing CA's own certificate, PEM: the rest of the chain an edge installs.
    fn certificate_pem(&self) -> String;
}

/// The issuing CA with its key at hand: the local adapter until the KMS one.
impl Issuer for Authority {
    fn issue(
        &self,
        request: &str,
        facility: &Facility,
        at: Timestamp,
    ) -> Result<(String, String), String> {
        let issued = self
            .issue_facility(request, facility, at)
            .map_err(|e| e.to_string())?;
        Ok((issued.pem(), issued.serial.to_string()))
    }

    fn certificate_pem(&self) -> String {
        Authority::certificate_pem(self)
    }
}

/// Why provisioning did not happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The code was refused.
    #[error(transparent)]
    Refused(#[from] CodeRefused),
    /// The certificate request was refused or unreadable.
    #[error("the certificate request was refused: {0}")]
    Request(String),
    /// The store failed, or a concurrent redemption won.
    #[error("the registry failed: {0}")]
    Store(String),
    /// A signed request from a node that is not an active one, or whose signature fails.
    #[error("not signed by an active node: {0}")]
    Unauthorized(String),
    /// A request that does not read.
    #[error("malformed: {0}")]
    Malformed(String),
    /// An escrow refused by the rules.
    #[error(transparent)]
    Escrow(#[from] EscrowRefused),
    /// No such node or recovery key.
    #[error("not found: {0}")]
    NotFound(String),
}

/// The JWS `typ` of a recovery-key escrow.
pub const RECOVERY_TYP: &str = "health-recovery+jwt";

/// The escrow an edge sends: its `keys` by volume, signed at `at` with its facility
/// intermediate's `key`, as a compact ES256 JWS.
pub fn recovery_jws(
    node: NodeId,
    keys: &[(RecoveryVolume, String)],
    key: &SigningKey,
    at: Timestamp,
) -> String {
    let header = URL_SAFE_NO_PAD.encode(json!({ "alg": "ES256", "typ": RECOVERY_TYP }).to_string());
    let payload = URL_SAFE_NO_PAD.encode(
        json!({
            "node": node.to_string(),
            "iat": at.as_second(),
            "keys": keys
                .iter()
                .map(|(volume, key)| json!({ "volume": volume.as_str(), "key": key }))
                .collect::<Vec<_>>(),
        })
        .to_string(),
    );
    let signature: Signature = key.sign(format!("{header}.{payload}").as_bytes());
    format!(
        "{header}.{payload}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    )
}

/// What a recovery key is wrapped against: its node and volume.
fn context(node: NodeId, volume: RecoveryVolume) -> String {
    format!("health-recovery/{node}/{}", volume.as_str())
}

fn failed(error: impl std::fmt::Display) -> Error {
    Error::Store(error.to_string())
}

/// A provisioned node and the chain to install: its facility intermediate, then the issuing CA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provisioned {
    /// The node.
    pub node: Node,
    /// The chain, PEM.
    pub chain_pem: String,
}

/// Central's node registry over SurrealDB.
#[derive(Clone)]
pub struct NodeRegistry {
    store: SurrealStore,
    clock: Arc<dyn Clock>,
    policy: ProvisioningPolicy,
    observer: String,
}

/// The sha-256 of a normalised code, hex: what the store keeps.
fn hash(code: &str) -> String {
    sha2::Sha256::digest(code.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl NodeRegistry {
    /// The registry over `store`, audited as `observer`.
    pub fn new(store: SurrealStore, clock: Arc<dyn Clock>, observer: impl Into<String>) -> Self {
        Self {
            store,
            clock,
            policy: ProvisioningPolicy::default(),
            observer: observer.into(),
        }
    }

    fn audit(&self, code: &str, severity: &str, by: &str, what: &str, detail: &str) -> Value {
        json!({
            "resourceType": "AuditEvent",
            "category": [{ "coding": [{ "system": "http://dicom.nema.org/resources/ontology/DCM", "code": "110137", "display": "User Security Attributes Changed" }] }],
            "code": { "coding": [{ "system": "urn:health:audit-event", "code": code }] },
            "action": "C",
            "severity": severity,
            "recorded": self.clock.now().to_string(),
            "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
            "agent": [{ "who": { "display": by }, "requestor": true }],
            "source": { "observer": { "display": self.observer } },
            "entity": [{
                "what": { "display": what },
                "detail": [{ "type": { "text": "change" }, "valueString": detail }],
            }],
        })
    }

    async fn begin(&self) -> Result<SurrealTx, Error> {
        self.store.begin().await.map_err(failed)
    }

    /// Issues a code for `facility`, by `issued_by`. Returns its text, shown once, and the code.
    pub async fn issue_code(
        &self,
        facility: &str,
        dns_names: Vec<String>,
        exemption: Option<String>,
        issued_by: &str,
    ) -> Result<(String, ProvisioningCode), Error> {
        let mut draw = [0u8; 5];
        rand_core::OsRng.fill_bytes(&mut draw);
        let text = self.policy.code(draw);
        let normalised = self
            .policy
            .normalise(&text)
            .ok_or_else(|| Error::Store("a code that does not read back".into()))?;
        let code = self.policy.issue(
            Uuid::now_v7(),
            facility.to_owned(),
            dns_names,
            exemption,
            issued_by.to_owned(),
            self.clock.now(),
        );
        let detail = match &code.exemption {
            Some(reason) => format!("for {facility}, exempt from encryption: {reason}"),
            None => format!("for {facility}"),
        };
        let audit = self.audit(
            "node-provisioning-code",
            if code.exemption.is_some() {
                "warning"
            } else {
                "notice"
            },
            issued_by,
            &format!("provisioning code {}", code.id),
            &detail,
        );
        let mut tx = self.begin().await?;
        tx.put_provisioning_code(
            &code.id.to_string(),
            &hash(&normalised),
            &serde_json::to_string(&code).map_err(failed)?,
        )
        .await
        .map_err(failed)?;
        health_audit::append(&mut tx, &audit)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        Ok((text, code))
    }

    /// Redeems `code` for an edge whose disks are `encrypted`, issuing its intermediate for
    /// `request` with `issuer`.
    pub async fn redeem(
        &self,
        code: &str,
        request: &str,
        encrypted: bool,
        issuer: &dyn Issuer,
    ) -> Result<Provisioned, Error> {
        let normalised = self.policy.normalise(code).ok_or(CodeRefused::Unknown)?;
        let now = self.clock.now();
        let mut tx = self.begin().await?;
        let mut stored: ProvisioningCode = match tx
            .provisioning_code_by_hash(&hash(&normalised))
            .await
            .map_err(failed)?
        {
            Some(json) => serde_json::from_str(&json).map_err(failed)?,
            None => return Err(CodeRefused::Unknown.into()),
        };
        let encryption = stored.redeemable(encrypted, now)?;
        let existing: Vec<Node> = tx
            .nodes_at(&stored.facility)
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect::<Result<_, _>>()?;
        let id = NodeId(Uuid::now_v7());
        let short_code =
            NodeKind::Edge.next_short_code(existing.iter().map(|n| n.short_code.as_str()));
        let facility = Facility {
            node_id: id.to_string(),
            dns_names: stored.dns_names.clone(),
        };
        let (intermediate, serial) = issuer
            .issue(request, &facility, now)
            .map_err(Error::Request)?;
        let intermediate_key = health_pki::inspect(&intermediate)
            .map(|inspected| URL_SAFE_NO_PAD.encode(inspected.public_key))
            .map_err(|e| Error::Request(e.to_string()))?;
        let node = Node {
            id,
            facility: stored.facility.clone(),
            kind: NodeKind::Edge,
            short_code,
            status: NodeStatus::Active,
            encryption,
            intermediate_serial: serial,
            intermediate_key,
            code: stored.id,
            provisioned_at: now,
        };
        stored.redeemed = Some((now, id));
        let exempt = matches!(node.encryption, Encryption::Exempt { .. });
        let audit = self.audit(
            "node-provisioned",
            if exempt { "warning" } else { "notice" },
            &format!("provisioning code {}", stored.id),
            &format!("node {id}"),
            &format!(
                "{} at {}, intermediate {}{}",
                node.short_code,
                node.facility,
                node.intermediate_serial,
                if exempt {
                    ", exempt from encryption"
                } else {
                    ""
                }
            ),
        );
        tx.put_node(
            &id.to_string(),
            &node.facility,
            &node.short_code,
            &stored.id.to_string(),
            &serde_json::to_string(&node).map_err(failed)?,
        )
        .await
        .map_err(failed)?;
        tx.update_provisioning_code(
            &stored.id.to_string(),
            &serde_json::to_string(&stored).map_err(failed)?,
        )
        .await
        .map_err(failed)?;
        health_audit::append(&mut tx, &audit)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        if exempt {
            tracing::warn!(alert = "encryption-exempt", node = %id, facility = %node.facility, "an edge was provisioned without full-disk encryption, under an exemption");
        }
        Ok(Provisioned {
            node,
            chain_pem: format!("{intermediate}{}", issuer.certificate_pem()),
        })
    }

    /// Every node at `facility`, decommissioned ones too.
    pub async fn nodes_at(&self, facility: &str) -> Result<Vec<Node>, Error> {
        let mut tx = self.begin().await?;
        tx.nodes_at(facility)
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect()
    }

    /// The active nodes running under an encryption exemption: a standing alert at central.
    pub async fn exempt(&self) -> Result<Vec<Node>, Error> {
        let mut tx = self.begin().await?;
        let nodes: Vec<Node> = tx
            .nodes()
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect::<Result<_, _>>()?;
        Ok(nodes
            .into_iter()
            .filter(|n| {
                n.status == NodeStatus::Active && matches!(n.encryption, Encryption::Exempt { .. })
            })
            .collect())
    }

    async fn node(&self, tx: &mut SurrealTx, id: &str) -> Result<Option<Node>, Error> {
        tx.node(id)
            .await
            .map_err(failed)?
            .map(|json| serde_json::from_str(&json).map_err(failed))
            .transpose()
    }

    /// Escrows the recovery keys in `jws`, an edge's [`recovery_jws`], wrapping each with
    /// `kms`. Returns the node and the volumes escrowed.
    pub async fn escrow(
        &self,
        jws: &str,
        kms: &dyn KeyWrap,
    ) -> Result<(NodeId, Vec<RecoveryVolume>), Error> {
        let malformed = |why: &str| Error::Malformed(why.to_owned());
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
        if head["alg"] != "ES256" || head["typ"] != RECOVERY_TYP || head.get("crit").is_some() {
            return Err(malformed("not a recovery-key escrow"));
        }
        let node_id = claims["node"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .map(NodeId)
            .ok_or_else(|| malformed("no node"))?;
        let signed_at = claims["iat"]
            .as_i64()
            .and_then(|s| Timestamp::from_second(s).ok())
            .ok_or_else(|| malformed("no signing time"))?;
        let keys: Vec<(RecoveryVolume, String)> = claims["keys"]
            .as_array()
            .ok_or_else(|| malformed("no keys"))?
            .iter()
            .map(|entry| {
                let volume = serde_json::from_value(entry["volume"].clone()).ok();
                let key = entry["key"].as_str().filter(|k| !k.is_empty());
                volume
                    .zip(key.map(str::to_owned))
                    .ok_or_else(|| malformed("a key without its volume"))
            })
            .collect::<Result<_, _>>()?;
        if keys.is_empty() {
            return Err(malformed("no keys"));
        }

        let now = self.clock.now();
        let mut tx = self.begin().await?;
        let unauthorized = |why: &str| Error::Unauthorized(why.to_owned());
        let node = self
            .node(&mut tx, &node_id.to_string())
            .await?
            .filter(|node| node.status == NodeStatus::Active)
            .ok_or_else(|| unauthorized("no active node by that id"))?;
        let key = URL_SAFE_NO_PAD
            .decode(&node.intermediate_key)
            .ok()
            .and_then(|sec1| VerifyingKey::from_sec1_bytes(&sec1).ok())
            .ok_or_else(|| unauthorized("the node has no key on record"))?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .ok()
            .and_then(|bytes| Signature::from_slice(&bytes).ok())
            .ok_or_else(|| unauthorized("unreadable signature"))?;
        key.verify(format!("{header}.{payload}").as_bytes(), &signature)
            .map_err(|_| unauthorized("the signature is not the node's"))?;

        let mut volumes = Vec::new();
        for (volume, secret) in &keys {
            let id = node_id.to_string();
            let current: Option<RecoveryKey> = tx
                .recovery_key(&id, volume.as_str())
                .await
                .map_err(failed)?
                .map(|(json, _)| serde_json::from_str(&json).map_err(failed))
                .transpose()?;
            let record = RecoveryKey::escrow(current.as_ref(), node_id, *volume, signed_at, now)?;
            let wrapped = kms
                .wrap(secret.as_bytes(), &context(node_id, *volume))
                .map_err(Error::Store)?;
            tx.put_recovery_key(
                &id,
                volume.as_str(),
                &serde_json::to_string(&record).map_err(failed)?,
                &URL_SAFE_NO_PAD.encode(wrapped),
            )
            .await
            .map_err(failed)?;
            volumes.push(*volume);
        }
        let names: Vec<&str> = volumes.iter().map(|v| v.as_str()).collect();
        let audit = self.audit(
            "node-recovery-escrowed",
            "notice",
            &format!("node {node_id}"),
            &format!("node {node_id}"),
            &format!("recovery keys escrowed for {}", names.join(", ")),
        );
        health_audit::append(&mut tx, &audit)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        Ok((node_id, volumes))
    }

    /// Reads out `node`'s recovery key for `volume`, by the central operations person `by`, who
    /// verified the caller as `verified` (ADR 0048 § Retrieval). Audited as a warning; the key is
    /// used from then on.
    pub async fn read_recovery(
        &self,
        node: NodeId,
        volume: RecoveryVolume,
        by: &str,
        verified: &str,
        kms: &dyn KeyWrap,
    ) -> Result<String, Error> {
        if by.trim().is_empty() || verified.trim().is_empty() {
            return Err(Error::Malformed(
                "a read-out needs who reads it and how the caller was verified".into(),
            ));
        }
        let mut tx = self.begin().await?;
        let id = node.to_string();
        let (json, wrapped) = tx
            .recovery_key(&id, volume.as_str())
            .await
            .map_err(failed)?
            .ok_or_else(|| {
                Error::NotFound(format!("no {} key for node {node}", volume.as_str()))
            })?;
        let mut record: RecoveryKey = serde_json::from_str(&json).map_err(failed)?;
        let secret = URL_SAFE_NO_PAD
            .decode(wrapped)
            .map_err(failed)
            .and_then(|wrapped| {
                kms.unwrap(&wrapped, &context(node, volume))
                    .map_err(Error::Store)
            })?;
        let secret = String::from_utf8(secret).map_err(failed)?;
        record.retrievals.push(Retrieval {
            at: self.clock.now(),
            by: by.to_owned(),
            verified: verified.to_owned(),
        });
        tx.update_recovery_key(
            &id,
            volume.as_str(),
            &serde_json::to_string(&record).map_err(failed)?,
        )
        .await
        .map_err(failed)?;
        let audit = self.audit(
            "node-recovery-read",
            "warning",
            by,
            &format!("node {node}"),
            &format!(
                "{} recovery key read out; caller verified: {verified}",
                volume.as_str()
            ),
        );
        health_audit::append(&mut tx, &audit)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        tracing::warn!(alert = "recovery-key-used", node = %node, volume = volume.as_str(), "a disk recovery key was read out; the edge must rotate it");
        Ok(secret)
    }

    /// The recovery keys read out and not yet replaced: a standing alert at central.
    pub async fn used_recovery_keys(&self) -> Result<Vec<RecoveryKey>, Error> {
        let mut tx = self.begin().await?;
        let keys: Vec<RecoveryKey> = tx
            .recovery_keys()
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect::<Result<_, _>>()?;
        Ok(keys.into_iter().filter(RecoveryKey::used).collect())
    }
}
