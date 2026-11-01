//! Caregiver delegation use cases (#113; patient-identity.md § Caregiver delegation).
//!
//! Staff record a caregiver against a dependent at the facility, verifying the documents in the
//! same step, and may revoke the grant or record the adolescent's consent. Each change writes
//! the grant ([`GrantStore`]) and its FHIR projections, the RelatedPerson and the Consent, in
//! one transaction, each version audited.

use std::future::Future;

use health_consent::{
    Authority, CaregiverGrant, Confidentiality, Day, GrantStore, Refused, Revoker, Scope,
    Verification,
};
use health_identifiers::{DerivationName, OpId, derive_id};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, StoreError, Transaction, WriteError, WriteRequest,
};
use health_time::Timestamp;
use serde_json::{Value, json};

/// The scope codes on a Consent's `provision.action`.
pub const SCOPE_SYSTEM: &str = "urn:health:caregiver-scope";
/// On a Consent: the caregiver's authority (`guardian` or `delegated`).
pub const AUTHORITY_EXTENSION: &str = "urn:health:extension/caregiver-authority";
/// On a Consent: whether the adolescent consents to their guardian seeing sensitive records.
pub const ADOLESCENT_EXTENSION: &str = "urn:health:extension/adolescent-consent";

/// Records each version a use case writes, in its transaction.
pub trait CaregiverAudit<T: Transaction>: Send + Sync {
    /// `json` was written in `tx` by `actor`, for `reason`.
    fn written(
        &self,
        tx: &mut T,
        actor: &str,
        reason: &str,
        json: &RawJson,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;
}

/// A caregiver use case failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaregiverError {
    /// No such grant.
    #[error("no such caregiver grant")]
    NotFound,
    /// The caregiver already holds a grant over this patient that is not revoked.
    #[error("this caregiver already has access to this patient")]
    AlreadyGranted,
    /// The request is incomplete.
    #[error("{0}")]
    Invalid(String),
    /// The rules refused it.
    #[error(transparent)]
    Refused(#[from] Refused),
    /// The store failed.
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for CaregiverError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<WriteError> for CaregiverError {
    fn from(error: WriteError) -> Self {
        Self::Store(error.to_string())
    }
}

/// A caregiver for staff to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGrant {
    /// The caregiver's phone Login Account.
    pub caregiver: String,
    /// Their verified phone, for the RelatedPerson.
    pub phone: String,
    /// The dependent's source Patient.
    pub patient: String,
    /// The relationship (`v3-RoleCode`, e.g. `MTH`, `FTH`, `GRD`).
    pub relationship: String,
    /// Guardian or delegated.
    pub authority: Authority,
    /// What the caregiver may do.
    pub scopes: Vec<Scope>,
    /// When the access ends.
    pub expires: Timestamp,
    /// The document staff checked.
    pub document: String,
}

/// The caregiver use cases over `store`.
pub struct Caregivers<S, A> {
    store: S,
    audit: A,
    policy: Confidentiality,
}

/// A grant's id: one per caregiver and patient.
pub fn grant_id(caregiver: &str, patient: &str) -> String {
    derive_id(&DerivationName::rule(
        format!("{caregiver}|{patient}"),
        "urn:health:rule/caregiver-grant",
    ))
    .to_string()
}

fn related_person_id(grant: &str) -> String {
    derive_id(&DerivationName::rule(
        grant,
        "urn:health:rule/caregiver-related-person",
    ))
    .to_string()
}

fn kind(name: &str) -> ResourceType {
    name.parse().expect("a FHIR resource type")
}

impl<S, A> Caregivers<S, A>
where
    S: Store,
    S::Tx: GrantStore,
    A: CaregiverAudit<S::Tx>,
{
    /// The use cases over `store`, under the confidentiality rules `policy`.
    pub fn new(store: S, audit: A, policy: Confidentiality) -> Self {
        Self {
            store,
            audit,
            policy,
        }
    }

    /// The confidentiality rules.
    pub fn policy(&self) -> &Confidentiality {
        &self.policy
    }

    async fn write(
        &self,
        tx: &mut S::Tx,
        actor: &str,
        reason: &str,
        resource: &str,
        id: &str,
        json: &Value,
    ) -> Result<(), CaregiverError> {
        let resource_type = kind(resource);
        let id = id
            .parse()
            .map_err(|_| CaregiverError::Invalid(format!("{resource} id {id} is not valid")))?;
        let precondition = match tx.read(&resource_type, id).await? {
            Some(stored) => Precondition::IfMatch(stored.version.version_id),
            None => Precondition::MustNotExist,
        };
        tx.write(WriteRequest {
            resource_type: resource_type.clone(),
            id,
            json: RawJson::from_validated(json.to_string()),
            op_id: OpId::new(),
            precondition,
        })
        .await?;
        let stored = tx
            .read(&resource_type, id)
            .await?
            .ok_or_else(|| CaregiverError::Store(format!("written {resource} is missing")))?;
        self.audit.written(tx, actor, reason, &stored.json).await?;
        Ok(())
    }

    async fn save(
        &self,
        tx: &mut S::Tx,
        grant: &CaregiverGrant,
        actor: &str,
        reason: &str,
        relationship: Option<(&str, &str)>,
    ) -> Result<(), CaregiverError> {
        tx.put_grant(grant).await?;
        if let Some((code, phone)) = relationship {
            let related = related_person(grant, code, phone);
            self.write(
                tx,
                actor,
                reason,
                "RelatedPerson",
                &grant.related_person,
                &related,
            )
            .await?;
        }
        self.write(tx, actor, reason, "Consent", &grant.id, &consent(grant))
            .await
    }

    /// Records a caregiver, verified by `staff` against the document they checked.
    pub async fn record(
        &self,
        staff: &str,
        new: NewGrant,
    ) -> Result<CaregiverGrant, CaregiverError> {
        if new.scopes.is_empty() {
            return Err(CaregiverError::Invalid(
                "a caregiver needs at least one scope".into(),
            ));
        }
        if new.document.trim().is_empty() {
            return Err(CaregiverError::Invalid(
                "recording a caregiver needs the document checked".into(),
            ));
        }
        let id = grant_id(&new.caregiver, &new.patient);
        let mut tx = self.store.begin().await?;
        if tx.grant(&id).await?.is_some_and(|g| g.revoked.is_none()) {
            return Err(CaregiverError::AlreadyGranted);
        }
        let grant = CaregiverGrant {
            related_person: related_person_id(&id),
            id,
            caregiver: new.caregiver,
            patient: new.patient,
            authority: new.authority,
            scopes: new.scopes,
            expires: new.expires,
            verification: Some(Verification::StaffChecked {
                staff: staff.to_owned(),
                document: new.document.trim().to_owned(),
            }),
            adolescent_consents: false,
            revoked: None,
        };
        self.save(
            &mut tx,
            &grant,
            staff,
            "caregiver-record",
            Some((&new.relationship, &new.phone)),
        )
        .await?;
        tx.commit().await?;
        Ok(grant)
    }

    /// Revokes grant `id` (see [`CaregiverGrant::revoke`] for who may). `birth_date` is the
    /// patient's.
    pub async fn revoke(
        &self,
        id: &str,
        by: Revoker,
        actor: &str,
        birth_date: Option<&str>,
        today: Day,
        now: Timestamp,
    ) -> Result<CaregiverGrant, CaregiverError> {
        let mut tx = self.store.begin().await?;
        let mut grant = tx.grant(id).await?.ok_or(CaregiverError::NotFound)?;
        let others = tx.grants_of_patient(&grant.patient).await?;
        let guardians: Vec<&CaregiverGrant> = others.iter().filter(|g| g.id != grant.id).collect();
        grant.revoke(by, &self.policy, birth_date, &guardians, today, now)?;
        self.save(&mut tx, &grant, actor, "caregiver-revoke", None)
            .await?;
        tx.commit().await?;
        Ok(grant)
    }

    /// Records the adolescent's consent to their guardian seeing their sensitive records, given
    /// or withdrawn.
    pub async fn adolescent_consent(
        &self,
        id: &str,
        consents: bool,
        actor: &str,
    ) -> Result<CaregiverGrant, CaregiverError> {
        let mut tx = self.store.begin().await?;
        let mut grant = tx.grant(id).await?.ok_or(CaregiverError::NotFound)?;
        if grant.authority != Authority::Guardian {
            return Err(CaregiverError::Invalid(
                "an adolescent's consent applies to a guardian's access".into(),
            ));
        }
        grant.adolescent_consent(consents);
        self.save(&mut tx, &grant, actor, "adolescent-consent", None)
            .await?;
        tx.commit().await?;
        Ok(grant)
    }

    /// The grants over `patient`.
    pub async fn of_patient(&self, patient: &str) -> Result<Vec<CaregiverGrant>, CaregiverError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.grants_of_patient(patient).await?)
    }

    /// The grants `caregiver` holds.
    pub async fn of_caregiver(
        &self,
        caregiver: &str,
    ) -> Result<Vec<CaregiverGrant>, CaregiverError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.grants_of_caregiver(caregiver).await?)
    }
}

/// The RelatedPerson for the dependent.
fn related_person(grant: &CaregiverGrant, relationship: &str, phone: &str) -> Value {
    json!({
        "resourceType": "RelatedPerson",
        "id": grant.related_person,
        "active": true,
        "patient": { "reference": format!("Patient/{}", grant.patient) },
        "relationship": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-RoleCode", "code": relationship }] }],
        "telecom": [{ "system": "phone", "value": phone, "use": "mobile" }],
    })
}

/// The grant's Consent: who, for whom, what and until when, how it was verified, and whether it
/// is in force.
fn consent(grant: &CaregiverGrant) -> Value {
    let verification: Vec<Value> = match &grant.verification {
        Some(Verification::StaffChecked { staff, document }) => vec![json!({
            "verified": true,
            "verificationType": { "text": document },
            "verifiedBy": { "display": staff },
        })],
        Some(Verification::Delivery { encounter }) => vec![json!({
            "verified": true,
            "verificationType": { "text": "delivery" },
            "verifiedWith": { "reference": format!("Encounter/{encounter}") },
        })],
        None => Vec::new(),
    };
    json!({
        "resourceType": "Consent",
        "id": grant.id,
        "status": if grant.revoked.is_some() { "inactive" } else { "active" },
        "category": [{ "text": "caregiver-access" }],
        "subject": { "reference": format!("Patient/{}", grant.patient) },
        "grantee": [{ "reference": format!("RelatedPerson/{}", grant.related_person) }],
        "period": { "end": grant.expires.to_string() },
        "decision": "permit",
        "verification": verification,
        "provision": [{
            "action": grant.scopes.iter().map(|s| json!({ "coding": [{ "system": SCOPE_SYSTEM, "code": s.code() }] })).collect::<Vec<_>>(),
        }],
        "extension": [
            { "url": AUTHORITY_EXTENSION, "valueCode": match grant.authority { Authority::Guardian => "guardian", Authority::Delegated => "delegated" } },
            { "url": ADOLESCENT_EXTENSION, "valueBoolean": grant.adolescent_consents },
        ],
    })
}
