use std::collections::BTreeSet;

use health_time::Timestamp;

use crate::break_glass::BreakGlass;
use crate::permission::{Interaction, Permissions};

/// A `meta.security` coding.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label {
    /// The code system, e.g. `http://terminology.hl7.org/CodeSystem/v3-ActCode`.
    pub system: String,
    /// The code, e.g. `HIV`.
    pub code: String,
}

impl Label {
    /// A label from its system and code.
    pub fn new(system: &str, code: &str) -> Self {
        Self {
            system: system.to_owned(),
            code: code.to_owned(),
        }
    }
}

/// A sensitivity label and the program assignment that opens records carrying it, from the
/// configuration package (InformationSensitivityPolicy codes such as HIV, TB, ETH, SDV, PSY, and
/// confidentiality `R`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelRule {
    /// The label.
    pub label: Label,
    /// The program a user must be assigned to.
    pub program: String,
}

/// A v3 PurposeOfUse code (`TREAT`, `ETREAT`, `HPAYMT`, …).
pub type PurposeOfUse = String;

/// Who is asking: what the session's permission snapshot or central's records say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    /// The account id.
    pub account: String,
    /// The facility the session is at.
    pub facility: String,
    /// The permissions resolved from the user's roles there.
    pub permissions: Permissions,
    /// Program assignments.
    pub programs: BTreeSet<String>,
    /// The purpose of use declared for this session or request, if any.
    pub purpose: Option<PurposeOfUse>,
    /// An active break-glass grant, if any.
    pub break_glass: Option<BreakGlass>,
}

/// One access to decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Access<'a> {
    /// The resource type.
    pub resource_type: &'a str,
    /// What is done to it.
    pub interaction: Interaction,
    /// The facility a facility-scope record belongs to; `None` for records that are not
    /// facility-scope (golden records, published definitions).
    pub facility: Option<&'a str>,
    /// The record's `meta.security` labels.
    pub labels: &'a [Label],
    /// The patient the record is about, if any.
    pub patient: Option<&'a str>,
}

/// What the caller must do with a permitted access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Obligation {
    /// The access relied on break-glass: audit it with `BTG` in the AuditEvent's authorization
    /// and break-the-glass labels on the access.
    BreakGlassUsed {
        /// The patient.
        patient: String,
    },
}

/// Why an access is denied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Deny {
    /// No permission maps to this interaction on this type.
    #[error("no permission for this interaction")]
    NoPermission,
    /// The record belongs to another facility.
    #[error("the record belongs to another facility")]
    OtherFacility,
    /// The record carries a label the user's program assignments do not open.
    #[error("the record carries a restricted label")]
    SensitiveLabel(Label),
    /// Sensitive records need a declared purpose of use.
    #[error("access to sensitive records needs a purpose of use")]
    PurposeRequired,
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Allowed, with what the caller must do.
    Permit(Vec<Obligation>),
    /// Refused.
    Deny(Deny),
}

/// What every search by a subject must exclude: records carrying any of these labels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFilter {
    /// Labels whose records are excluded, sorted.
    pub excluded_labels: Vec<Label>,
}

/// The policy decision point, configured with the country's label rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pdp {
    rules: Vec<LabelRule>,
}

impl Pdp {
    /// A PDP over `rules`.
    pub fn new(rules: Vec<LabelRule>) -> Self {
        Self { rules }
    }

    fn sensitive<'a>(&'a self, labels: &'a [Label]) -> impl Iterator<Item = &'a Label> + 'a {
        labels
            .iter()
            .filter(|l| self.rules.iter().any(|r| r.label == **l))
    }

    fn opens(&self, subject: &Subject, label: &Label) -> bool {
        self.rules
            .iter()
            .any(|r| r.label == *label && subject.programs.contains(&r.program))
    }

    /// Decides one access at `now`.
    pub fn decide(&self, subject: &Subject, access: &Access<'_>, now: Timestamp) -> Decision {
        if !subject
            .permissions
            .allows(access.resource_type, access.interaction)
        {
            return Decision::Deny(Deny::NoPermission);
        }
        let glass = access
            .patient
            .zip(subject.break_glass.as_ref())
            .filter(|(patient, grant)| grant.covers(patient, now))
            .map(|(patient, _)| patient);
        let mut used_glass = false;
        if access.facility.is_some_and(|f| f != subject.facility) {
            if glass.is_none() {
                return Decision::Deny(Deny::OtherFacility);
            }
            used_glass = true;
        }
        let mut sensitive = false;
        for label in self.sensitive(access.labels) {
            sensitive = true;
            if !self.opens(subject, label) {
                if glass.is_none() {
                    return Decision::Deny(Deny::SensitiveLabel(label.clone()));
                }
                used_glass = true;
            }
        }
        // Break-glass is itself an emergency purpose; otherwise sensitive access declares one.
        if sensitive && subject.purpose.is_none() && glass.is_none() {
            return Decision::Deny(Deny::PurposeRequired);
        }
        match glass.filter(|_| used_glass) {
            Some(patient) => Decision::Permit(vec![Obligation::BreakGlassUsed {
                patient: patient.to_owned(),
            }]),
            None => Decision::Permit(Vec::new()),
        }
    }

    /// What a search of `resource_type` must exclude. `patient` is the patient the search is
    /// confined to, if it is: an active break-glass grant for that patient lifts the exclusions.
    /// A search across patients never relies on break-glass.
    pub fn search_filter(
        &self,
        subject: &Subject,
        resource_type: &str,
        patient: Option<&str>,
        now: Timestamp,
    ) -> Result<SearchFilter, Deny> {
        if !subject
            .permissions
            .allows(resource_type, Interaction::Search)
        {
            return Err(Deny::NoPermission);
        }
        let glass = patient
            .zip(subject.break_glass.as_ref())
            .is_some_and(|(p, grant)| grant.covers(p, now));
        if glass {
            return Ok(SearchFilter::default());
        }
        let mut excluded: Vec<Label> = self
            .rules
            .iter()
            .map(|r| &r.label)
            .filter(|label| subject.purpose.is_none() || !self.opens(subject, label))
            .cloned()
            .collect();
        excluded.sort();
        excluded.dedup();
        Ok(SearchFilter {
            excluded_labels: excluded,
        })
    }
}
