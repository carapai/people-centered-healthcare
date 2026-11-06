//! Patient requests (patient-identity.md § Patient-initiated workflows; MASTER_PROMPT §60; #114).
//! **A request is a proposal; it never becomes an order by itself.** A member of staff accepts it,
//! writing the order or booking, or declines it with a reason; the patient may withdraw it while
//! it is pending.
//!
//! | Request | Proposal | Accepted | Declined or withdrawn |
//! |---|---|---|---|
//! | A refill of a prescription | MedicationRequest `intent=proposal` (`active`) | `completed`, and a prescription `basedOn` it | `cancelled` |
//! | A test or care | ServiceRequest `intent=proposal` (`active`) | `completed`, and an order `basedOn` it | `revoked` |
//! | An appointment | Appointment `proposed` | `booked`, at the time staff set | `cancelled` |

/// What a patient asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// A refill of one of their prescriptions.
    Refill,
    /// A test or other care.
    Care,
    /// An appointment.
    Appointment,
}

impl RequestKind {
    /// The resource type of its proposal.
    pub fn resource_type(self) -> &'static str {
        match self {
            Self::Refill => "MedicationRequest",
            Self::Care => "ServiceRequest",
            Self::Appointment => "Appointment",
        }
    }

    /// The kind of request a proposal of `resource_type` is.
    pub fn of(resource_type: &str) -> Option<Self> {
        match resource_type {
            "MedicationRequest" => Some(Self::Refill),
            "ServiceRequest" => Some(Self::Care),
            "Appointment" => Some(Self::Appointment),
            _ => None,
        }
    }

    /// The proposal's status while pending.
    pub fn pending(self) -> &'static str {
        match self {
            Self::Appointment => "proposed",
            Self::Refill | Self::Care => "active",
        }
    }

    /// The proposal's status once accepted.
    pub fn accepted(self) -> &'static str {
        match self {
            Self::Appointment => "booked",
            Self::Refill | Self::Care => "completed",
        }
    }

    /// The proposal's status once declined or withdrawn.
    pub fn closed(self) -> &'static str {
        match self {
            Self::Care => "revoked",
            Self::Refill | Self::Appointment => "cancelled",
        }
    }
}

/// Where a request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestState {
    /// Waiting for staff.
    Pending,
    /// Accepted: the order or booking was made.
    Accepted,
    /// Declined by staff, or withdrawn by the patient.
    Closed,
}

impl RequestState {
    /// The state a proposal's `status` gives, for a request of `kind`.
    pub fn of(kind: RequestKind, status: &str) -> Option<Self> {
        if status == kind.pending() {
            Some(Self::Pending)
        } else if status == kind.accepted() {
            Some(Self::Accepted)
        } else if status == kind.closed() {
            Some(Self::Closed)
        } else {
            None
        }
    }
}

/// Why a request step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestRefused {
    /// Only a pending request is accepted, declined or withdrawn.
    #[error("the request is no longer pending")]
    NotPending,
    /// A request needs what it asks for.
    #[error("the request needs {0}")]
    Missing(&'static str),
    /// A declined request says why.
    #[error("a declined request says why")]
    NoReason,
}

/// Whether a request in `state` may be accepted, declined or withdrawn.
pub fn may_close(state: RequestState) -> Result<(), RequestRefused> {
    if state == RequestState::Pending {
        Ok(())
    } else {
        Err(RequestRefused::NotPending)
    }
}

/// Whether a decline gives its reason.
pub fn decline_reason(reason: &str) -> Result<&str, RequestRefused> {
    let reason = reason.trim();
    if reason.is_empty() {
        Err(RequestRefused::NoReason)
    } else {
        Ok(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_its_proposal_and_statuses() {
        for kind in [
            RequestKind::Refill,
            RequestKind::Care,
            RequestKind::Appointment,
        ] {
            assert_eq!(RequestKind::of(kind.resource_type()), Some(kind));
            assert_eq!(
                RequestState::of(kind, kind.pending()),
                Some(RequestState::Pending)
            );
            assert_eq!(
                RequestState::of(kind, kind.accepted()),
                Some(RequestState::Accepted)
            );
            assert_eq!(
                RequestState::of(kind, kind.closed()),
                Some(RequestState::Closed)
            );
            assert_eq!(RequestState::of(kind, "entered-in-error"), None);
        }
        assert_eq!(RequestKind::of("Task"), None);
        assert_eq!(RequestKind::Appointment.accepted(), "booked");
        assert_eq!(RequestKind::Care.closed(), "revoked");
    }

    #[test]
    fn only_a_pending_request_closes_and_a_decline_says_why() {
        assert_eq!(may_close(RequestState::Pending), Ok(()));
        assert_eq!(
            may_close(RequestState::Accepted),
            Err(RequestRefused::NotPending)
        );
        assert_eq!(
            may_close(RequestState::Closed),
            Err(RequestRefused::NotPending)
        );
        assert_eq!(decline_reason("  not due yet "), Ok("not due yet"));
        assert_eq!(decline_reason(" "), Err(RequestRefused::NoReason));
    }
}
