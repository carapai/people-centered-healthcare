//! Caregiver delegation (patient-identity.md § Caregiver delegation; ADR 0029; §96).
//!
//! A caregiver reaches a dependent's record only through a [`CaregiverGrant`]: a RelatedPerson
//! for the dependent, a Consent with a [`Scope`] and an expiry, and a [`Verification`].
//! [`CaregiverGrant::decide`] is the rule the policy decision point applies with the request's
//! confidentiality labels:
//! - a guardian's access ends at the age of majority (default 18);
//! - from the adolescent age (default 12), records with a sensitive label (HIV, SRH, mental
//!   health) are excluded from a guardian's view unless the adolescent consents;
//! - a revoked, expired or unverified grant gives nothing.
//!
//! Ages are reckoned from the earliest birth date a partial date allows, so an uncertain age
//! ends guardianship and starts adolescent confidentiality as early as possible. A dependent
//! with no known birth date gives a guardian nothing.

mod day;

use health_time::Timestamp;

pub use day::{Day, InvalidDay};

/// What a grant lets the caregiver do (the Consent's scope).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// See the dependent's released records.
    ViewRecords,
    /// Propose appointments.
    BookAppointments,
    /// Request refills.
    RequestRefills,
    /// Receive messages about the dependent.
    ReceiveMessages,
}

impl Scope {
    /// Every scope.
    pub const ALL: [Self; 4] = [
        Self::ViewRecords,
        Self::BookAppointments,
        Self::RequestRefills,
        Self::ReceiveMessages,
    ];

    /// The scope's code.
    pub fn code(self) -> &'static str {
        match self {
            Self::ViewRecords => "view-records",
            Self::BookAppointments => "book-appointments",
            Self::RequestRefills => "request-refills",
            Self::ReceiveMessages => "receive-messages",
        }
    }

    /// The scope with `code`.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.code() == code)
    }
}

/// Where the caregiver's authority comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    /// A parent or guardian of a minor: it ends at the age of majority, and adolescent
    /// confidentiality applies.
    Guardian,
    /// Delegated by the patient themselves, once of age (e.g. an adult child caring for a
    /// parent).
    Delegated,
}

/// How the relationship was verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verification {
    /// Facility staff checked documents.
    StaffChecked {
        /// The member of staff.
        staff: String,
        /// What they checked, e.g. `birth-certificate`.
        document: String,
    },
    /// A mother–newborn link recorded at a facility delivery: `level3` automatically, one
    /// grant per baby of a multiple birth.
    Delivery {
        /// The delivery Encounter.
        encounter: String,
    },
}

/// Who revoked a grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revoker {
    /// The patient themselves.
    Patient,
    /// A guardian of the patient (a caregiver with guardian authority over them).
    Guardian {
        /// The guardian's grant over the patient.
        grant: String,
    },
    /// A member of staff.
    Staff {
        /// The member of staff.
        staff: String,
    },
}

/// A revocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revoked {
    /// Who.
    pub by: Revoker,
    /// When.
    pub at: Timestamp,
}

/// The confidentiality rules, from the country configuration package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confidentiality {
    /// Guardian access ends at this age.
    pub age_of_majority: u32,
    /// From this age, sensitive records are excluded from guardians unless the adolescent
    /// consents.
    pub adolescent_age: u32,
    /// The sensitive labels (`meta.security` system and code): HIV, SRH, mental health.
    pub sensitive: Vec<(String, String)>,
}

impl Default for Confidentiality {
    /// Majority at 18, adolescent confidentiality from 12; the sensitive labels come from the
    /// configuration package and are empty here.
    fn default() -> Self {
        Self {
            age_of_majority: 18,
            adolescent_age: 12,
            sensitive: Vec::new(),
        }
    }
}

impl Confidentiality {
    fn is_sensitive(&self, label: &(String, String)) -> bool {
        self.sensitive.contains(label)
    }
}

/// What the caregiver is asking to do, for whom, now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<'a> {
    /// The scope used.
    pub scope: Scope,
    /// The dependent's birth date (`Patient.birthDate`; partial dates allowed), if known.
    pub birth_date: Option<&'a str>,
    /// The record's labels, for [`Scope::ViewRecords`].
    pub labels: &'a [(String, String)],
    /// Today, in the node's time zone.
    pub today: Day,
    /// Now.
    pub now: Timestamp,
}

/// Why a caregiver may not do it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// The relationship is not verified yet.
    #[error("the caregiver relationship is not verified")]
    Unverified,
    /// The grant was revoked.
    #[error("the caregiver's access was revoked")]
    Revoked,
    /// The grant has expired.
    #[error("the caregiver's access has expired")]
    Expired,
    /// The grant does not include this scope.
    #[error("the caregiver's access does not include {0:?}")]
    OutOfScope(Scope),
    /// The dependent has reached the age of majority (or their age is unknown): a guardian has
    /// no access.
    #[error("the patient is of age; a guardian has no access")]
    OfAge,
    /// A sensitive record of an adolescent, who has not consented to the guardian seeing it.
    #[error("this record is confidential to the adolescent")]
    Confidential,
    /// The revoker may not revoke this grant.
    #[error("{0}")]
    MayNotRevoke(&'static str),
}

/// A caregiver's access to one dependent: the RelatedPerson, its Consent and the verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaregiverGrant {
    /// The grant's id (its Consent).
    pub id: String,
    /// The caregiver's Person (their Login Account's).
    pub caregiver: String,
    /// The RelatedPerson for the dependent.
    pub related_person: String,
    /// The dependent's source Patient.
    pub patient: String,
    /// Guardian or delegated.
    pub authority: Authority,
    /// What the caregiver may do.
    pub scopes: Vec<Scope>,
    /// When the Consent ends.
    pub expires: Timestamp,
    /// How the relationship was verified, once it is.
    pub verification: Option<Verification>,
    /// Whether the adolescent consents to the guardian seeing their sensitive records.
    pub adolescent_consents: bool,
    /// The revocation, if any.
    pub revoked: Option<Revoked>,
}

impl CaregiverGrant {
    /// A mother's grant over her newborn, recorded at a facility delivery: verified at once,
    /// with every scope, until the newborn's majority (`majority`, computed by the caller from
    /// the birth). One per baby of a multiple birth.
    pub fn at_delivery(
        id: &str,
        mother: &str,
        related_person: &str,
        newborn: &str,
        encounter: &str,
        majority: Timestamp,
    ) -> Self {
        Self {
            id: id.to_owned(),
            caregiver: mother.to_owned(),
            related_person: related_person.to_owned(),
            patient: newborn.to_owned(),
            authority: Authority::Guardian,
            scopes: Scope::ALL.to_vec(),
            expires: majority,
            verification: Some(Verification::Delivery {
                encounter: encounter.to_owned(),
            }),
            adolescent_consents: false,
            revoked: None,
        }
    }

    /// Whether the caregiver may do `request`.
    pub fn decide(&self, policy: &Confidentiality, request: &Request<'_>) -> Result<(), Refused> {
        if self.revoked.is_some() {
            return Err(Refused::Revoked);
        }
        if self.verification.is_none() {
            return Err(Refused::Unverified);
        }
        if request.now >= self.expires {
            return Err(Refused::Expired);
        }
        if !self.scopes.contains(&request.scope) {
            return Err(Refused::OutOfScope(request.scope));
        }
        if self.authority == Authority::Guardian {
            let age = request
                .birth_date
                .and_then(|b| Day::earliest(b).ok())
                .map(|birth| birth.age_on(request.today));
            let Some(age) = age.filter(|a| *a < policy.age_of_majority) else {
                return Err(Refused::OfAge);
            };
            if request.scope == Scope::ViewRecords
                && age >= policy.adolescent_age
                && !self.adolescent_consents
                && request.labels.iter().any(|l| policy.is_sensitive(l))
            {
                return Err(Refused::Confidential);
            }
        }
        Ok(())
    }

    /// Records the staff verification of the relationship.
    pub fn verify(&mut self, verification: Verification) {
        self.verification = Some(verification);
    }

    /// The adolescent's consent to their guardian seeing their sensitive records, given or
    /// withdrawn.
    pub fn adolescent_consent(&mut self, consents: bool) {
        self.adolescent_consents = consents;
    }

    /// Revokes the grant. The patient may revoke a guardian's grant once of age (and a
    /// delegation at any time); a guardian with authority over the patient, or staff, at any
    /// time. `guardians` are the grants with guardian authority over the patient that are in
    /// force. Revoking twice keeps the first.
    pub fn revoke(
        &mut self,
        by: Revoker,
        policy: &Confidentiality,
        birth_date: Option<&str>,
        guardians: &[&CaregiverGrant],
        today: Day,
        now: Timestamp,
    ) -> Result<(), Refused> {
        if self.revoked.is_some() {
            return Ok(());
        }
        match &by {
            Revoker::Patient if self.authority == Authority::Guardian => {
                // Of age by the latest birth date the record allows: a minor cannot end their
                // guardian's grant on an uncertain age.
                let age = birth_date
                    .and_then(|b| Day::latest(b).ok())
                    .map(|birth| birth.age_on(today));
                if age.is_none_or(|a| a < policy.age_of_majority) {
                    return Err(Refused::MayNotRevoke(
                        "a patient revokes a guardian's access once of age",
                    ));
                }
            }
            Revoker::Guardian { grant } => {
                let has_authority = guardians.iter().any(|g| {
                    g.id == *grant
                        && g.patient == self.patient
                        && g.authority == Authority::Guardian
                        && g.revoked.is_none()
                        && g.verification.is_some()
                        && now < g.expires
                });
                if !has_authority {
                    return Err(Refused::MayNotRevoke(
                        "only a guardian with authority over the patient may revoke",
                    ));
                }
            }
            Revoker::Patient | Revoker::Staff { .. } => {}
        }
        self.revoked = Some(Revoked { by, at: now });
        Ok(())
    }
}
