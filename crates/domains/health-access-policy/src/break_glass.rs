use health_time::{SignedDuration, Timestamp};

use crate::decision::Subject;

/// The permission a role must map to for its holder to break glass ("may break glass" in the
/// configuration package).
pub const BREAK_GLASS_PERMISSION: &str = "break-glass";

/// The `v3-ActReason` code a break-glass request must carry.
pub const BTG: &str = "BTG";

/// The rules for break-glass (offline-auth.md § Break-glass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakGlassPolicy {
    /// How long a grant lasts; it is never extended.
    pub duration: SignedDuration,
}

impl Default for BreakGlassPolicy {
    fn default() -> Self {
        Self {
            duration: SignedDuration::from_hours(1),
        }
    }
}

/// An active break-glass grant: one patient, until `expires_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakGlass {
    /// The patient whose records it opens.
    pub patient: String,
    /// When it was granted.
    pub granted_at: Timestamp,
    /// When it ends.
    pub expires_at: Timestamp,
}

impl BreakGlass {
    /// Whether it covers `patient` at `now`.
    pub fn covers(&self, patient: &str, now: Timestamp) -> bool {
        self.patient == patient && now < self.expires_at
    }
}

/// What a grant reports: an AuditEvent with `BTG` in its authorization, an immediate alert to
/// the facility in-charge, and a privacy-officer review within 72 hours of reaching central.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakGlassEvent {
    /// Who broke glass.
    pub account: String,
    /// At which facility.
    pub facility: String,
    /// For whom.
    pub patient: String,
    /// The reason code (`BTG`).
    pub reason: String,
    /// The user's free-text justification.
    pub justification: String,
    /// When.
    pub at: Timestamp,
    /// Until when.
    pub expires_at: Timestamp,
}

/// Why break-glass was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BreakGlassRefused {
    /// The user's roles may not break glass.
    #[error("this role may not break glass")]
    NotPermitted,
    /// The reason is not `BTG`, or the justification is empty.
    #[error("break-glass needs the BTG reason and a justification")]
    MissingReason,
    /// A grant is still active: it cannot be extended or moved to another patient.
    #[error("a break-glass grant is already active")]
    AlreadyActive,
}

impl BreakGlassPolicy {
    /// Grants break-glass for `patient` at `now`, with the event to audit and alert on.
    pub fn grant(
        &self,
        subject: &Subject,
        patient: &str,
        reason: &str,
        justification: &str,
        now: Timestamp,
    ) -> Result<(BreakGlass, BreakGlassEvent), BreakGlassRefused> {
        if !subject.permissions.has(BREAK_GLASS_PERMISSION) {
            return Err(BreakGlassRefused::NotPermitted);
        }
        if reason != BTG || justification.trim().is_empty() {
            return Err(BreakGlassRefused::MissingReason);
        }
        if subject
            .break_glass
            .as_ref()
            .is_some_and(|b| now < b.expires_at)
        {
            return Err(BreakGlassRefused::AlreadyActive);
        }
        let expires_at = now.checked_add(self.duration).unwrap_or(now);
        let grant = BreakGlass {
            patient: patient.to_owned(),
            granted_at: now,
            expires_at,
        };
        let event = BreakGlassEvent {
            account: subject.account.clone(),
            facility: subject.facility.clone(),
            patient: patient.to_owned(),
            reason: reason.to_owned(),
            justification: justification.trim().to_owned(),
            at: now,
            expires_at,
        };
        Ok((grant, event))
    }
}
