//! Break-glass grants on the wire (#224; offline-auth.md § Break-glass): a grant made at a node
//! travels up as an append-only record, and the privacy officer's 72-hour review starts when it
//! reaches central, not when it was made at a node that may have been offline for days.

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// One break-glass grant, as the node that made it holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakGlassGrant {
    /// The grant's id at the node that made it.
    pub id: String,
    /// The session it was granted to.
    pub session: String,
    /// Who broke glass.
    pub account: String,
    /// At which facility.
    pub facility: String,
    /// For whom (`Patient/{id}`).
    pub patient: String,
    /// The reason code (`BTG`).
    pub reason: String,
    /// The user's justification.
    pub justification: String,
    /// When it was granted.
    pub at: Timestamp,
    /// When it ended.
    pub expires_at: Timestamp,
    /// Whether the facility's in-charge acknowledged the alert.
    pub acknowledged: bool,
}

/// How long the privacy officer has to review a grant once it reaches central.
pub const REVIEW_WITHIN_HOURS: i64 = 72;
