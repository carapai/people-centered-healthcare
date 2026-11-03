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

mod routes;

use std::sync::Arc;

use health_identity::{
    CodeRefused, Encryption, Node, NodeId, NodeKind, NodeStatus, ProvisioningCode,
    ProvisioningPolicy,
};
use health_persistence::{Store, Transaction};
use health_pki::{Authority, Facility};
use health_surrealdb::{SurrealStore, SurrealTx};
use health_time::{Clock, Timestamp};
use rand_core::RngCore;
use serde_json::{Value, json};
use sha2::Digest as _;
use uuid::Uuid;

pub use routes::router;

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
        let node = Node {
            id,
            facility: stored.facility.clone(),
            kind: NodeKind::Edge,
            short_code,
            status: NodeStatus::Active,
            encryption,
            intermediate_serial: serial,
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
}
