//! A Login Account's `Person` and its links to source Patients, each at an assurance level
//! (ADR 0029; patient-identity.md § Self-registration).

use health_time::Timestamp;

/// How sure we are that the account holder is the person (FHIR `IdentityAssuranceLevel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Assurance {
    /// A one-time code to the phone: it shows who holds the phone, not who the person is.
    Level1,
    /// A NIRA NIN check, or an in-person check of an ID document.
    Level3,
}

impl Assurance {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Level1 => "level1",
            Self::Level3 => "level3",
        }
    }
}

/// What a verification rests on (a FHIR `VerificationResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// A one-time code to the account's phone.
    PhoneOtp,
    /// A NIN checked with NIRA.
    Nira {
        /// The verified NIN.
        nin: String,
    },
    /// Staff checked an ID document in person.
    InPerson {
        /// The member of staff.
        staff: String,
        /// The document they checked, e.g. `national-id`.
        document: String,
    },
}

impl Evidence {
    /// The assurance it gives.
    pub fn assurance(&self) -> Assurance {
        match self {
            Self::PhoneOtp => Assurance::Level1,
            Self::Nira { .. } | Self::InPerson { .. } => Assurance::Level3,
        }
    }
}

/// A verification of the account holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    /// What it rests on.
    pub evidence: Evidence,
    /// When.
    pub at: Timestamp,
}

/// A `Person.link`: a source Patient the account may act as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonLink {
    /// The source Patient's id.
    pub patient: String,
    /// `Person.link.assurance`.
    pub assurance: Assurance,
    /// Whether the Patient was created by this account's self-registration (provisional until
    /// `level3`), rather than an existing Patient.
    pub self_registered: bool,
}

/// What an account may see of a linked Patient's record in the portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalAccess {
    /// Not linked: nothing.
    None,
    /// Below `level3`: only what the patient submitted themselves.
    SelfSubmitted,
    /// `level3`: released clinical records too (the PDP and sensitivity labels still apply).
    Records,
}

/// Where a record came from, for [`PortalAccess::may_see`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Submitted by the patient through the portal (tagged `patient-reported`).
    SelfSubmitted,
    /// Recorded by a facility.
    Facility,
}

impl PortalAccess {
    /// Whether a record of `origin` may be shown.
    pub fn may_see(self, origin: Origin) -> bool {
        match (self, origin) {
            (Self::None, _) => false,
            (Self::SelfSubmitted, Origin::SelfSubmitted) => true,
            (Self::SelfSubmitted, Origin::Facility) => false,
            (Self::Records, _) => true,
        }
    }
}

/// Why a link was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PersonLinkRefused {
    /// Linking to an existing Patient needs `level3`; a phone account is never enough.
    #[error("linking to an existing patient needs a level3 verification")]
    NeedsLevel3,
    /// The account is already linked to that Patient.
    #[error("already linked to {0}")]
    AlreadyLinked(String),
    /// The account has no link to that Patient.
    #[error("not linked to {0}")]
    NotLinked(String),
}

/// The `Person` of one Login Account. It is linked to Patients only by the methods here: a
/// matching phone number, or any automatic match below `level3`, never links it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// The Person's id.
    pub id: String,
    /// The Login Account it belongs to.
    pub account: String,
    /// What the account may act as.
    pub links: Vec<PersonLink>,
}

impl Person {
    /// A Person with no links.
    pub fn new(id: &str, account: &str) -> Self {
        Self {
            id: id.to_owned(),
            account: account.to_owned(),
            links: Vec::new(),
        }
    }

    fn link(&self, patient: &str) -> Option<&PersonLink> {
        self.links.iter().find(|link| link.patient == patient)
    }

    /// Links the source Patient the account's own self-registration created, at the assurance
    /// the account has: its data is what the patient submitted.
    pub fn link_self_registered(
        &mut self,
        patient: &str,
        assurance: Assurance,
    ) -> Result<(), PersonLinkRefused> {
        if self.link(patient).is_some() {
            return Err(PersonLinkRefused::AlreadyLinked(patient.to_owned()));
        }
        self.links.push(PersonLink {
            patient: patient.to_owned(),
            assurance,
            self_registered: true,
        });
        Ok(())
    }

    /// Links an existing source Patient: only on a `level3` verification.
    pub fn link_existing(
        &mut self,
        patient: &str,
        verification: &Verification,
    ) -> Result<(), PersonLinkRefused> {
        if verification.evidence.assurance() < Assurance::Level3 {
            return Err(PersonLinkRefused::NeedsLevel3);
        }
        if self.link(patient).is_some() {
            return Err(PersonLinkRefused::AlreadyLinked(patient.to_owned()));
        }
        self.links.push(PersonLink {
            patient: patient.to_owned(),
            assurance: Assurance::Level3,
            self_registered: false,
        });
        Ok(())
    }

    /// Raises a link's assurance after a later verification; it never lowers one.
    pub fn verify(
        &mut self,
        patient: &str,
        verification: &Verification,
    ) -> Result<Assurance, PersonLinkRefused> {
        let link = self
            .links
            .iter_mut()
            .find(|link| link.patient == patient)
            .ok_or_else(|| PersonLinkRefused::NotLinked(patient.to_owned()))?;
        link.assurance = link.assurance.max(verification.evidence.assurance());
        Ok(link.assurance)
    }

    /// What the account may see of `patient`'s record.
    pub fn access(&self, patient: &str) -> PortalAccess {
        match self.link(patient) {
            None => PortalAccess::None,
            Some(link) if link.assurance >= Assurance::Level3 => PortalAccess::Records,
            Some(_) => PortalAccess::SelfSubmitted,
        }
    }
}
