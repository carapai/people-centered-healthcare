//! Identity & Access use cases: the request's [`Subject`] built from the user's permission
//! snapshot ([`subject_of`]), and the policy decision point applied to searches, so restricted
//! records are excluded inside the database query (offline-auth.md § Authorization model:
//! "repository searches receive its decision as a query filter").

use health_access_policy::{Deny, Label, Pdp, Permissions, PurposeOfUse, Subject};
use health_fhir_search::{Clause, ParamType, SearchPolicy, SearchValue, Test};
use health_identity::snapshot::{PermissionSnapshot, RolePermissions};
use health_time::Timestamp;

/// The subject of a request by the snapshot's user at its facility (ADR 0056): the permissions
/// of their roles there, their program assignments and the declared purpose of use. The
/// snapshot must already be verified and current; break-glass grants are added by the caller.
pub fn subject_of(
    snapshot: &PermissionSnapshot,
    roles: &RolePermissions,
    purpose: PurposeOfUse,
) -> Subject {
    Subject {
        account: snapshot.account.to_string(),
        facility: snapshot.facility.clone(),
        permissions: Permissions(roles.resolve(snapshot)),
        programs: snapshot.programs.iter().cloned().collect(),
        purpose: Some(purpose),
        break_glass: None,
    }
}

/// The `meta.security` search parameter (`Resource-security`).
pub const SECURITY_PARAM: &str = "_security";

/// A search policy for one subject's request: every search excludes the records whose labels the
/// subject may not see, by `_security:not=…`, which also keeps records with no labels.
#[derive(Debug, Clone)]
pub struct SubjectSearchPolicy {
    pdp: Pdp,
    subject: Subject,
    patient: Option<String>,
    now: Timestamp,
}

impl SubjectSearchPolicy {
    /// The policy for `subject` at `now`. `patient` is the patient the request's searches are
    /// confined to, if any (a patient compartment or a `patient=` search): only then can an
    /// active break-glass grant lift the exclusions.
    pub fn new(pdp: Pdp, subject: Subject, patient: Option<String>, now: Timestamp) -> Self {
        Self {
            pdp,
            subject,
            patient,
            now,
        }
    }

    /// Whether the subject may search `resource_type` at all; the caller answers 403 otherwise.
    pub fn check(&self, resource_type: &str) -> Result<(), Deny> {
        self.pdp
            .search_filter(
                &self.subject,
                resource_type,
                self.patient.as_deref(),
                self.now,
            )
            .map(|_| ())
    }
}

fn token(label: &Label) -> SearchValue {
    SearchValue::Token {
        system: Some(label.system.clone()),
        code: Some(label.code.clone()),
    }
}

impl SearchPolicy for SubjectSearchPolicy {
    fn clauses(&self, resource_type: &str) -> Vec<Clause> {
        match self.pdp.search_filter(
            &self.subject,
            resource_type,
            self.patient.as_deref(),
            self.now,
        ) {
            Ok(filter) if filter.excluded_labels.is_empty() => Vec::new(),
            Ok(filter) => vec![Clause {
                param: SECURITY_PARAM.into(),
                param_type: ParamType::Token,
                test: Test::NoneOf(filter.excluded_labels.iter().map(token).collect()),
            }],
            // No search permission: a clause no record satisfies, so a caller that skipped
            // `check` still returns nothing.
            Err(_) => vec![Clause {
                param: "_id".into(),
                param_type: ParamType::Token,
                test: Test::Missing(true),
            }],
        }
    }
}
