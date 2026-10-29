//! Identifiers (docs/architecture/identifiers.md, ADRs 0013 and 0014).
//!
//! - Every id is a typed newtype over a UUID, written lowercase and hyphenated. Bare `Uuid` or
//!   `String` ids never cross a crate API; Modules declare their own with [`define_id!`].
//! - Records derived deterministically from another record get a UUIDv5 under
//!   [`PLATFORM_NAMESPACE`], so replaying a derivation on any node converges on one id.
//! - Human-readable numbers are `{facility}-{node}-{sequence}`, minted from a node-local counter.

mod derive;
mod id;
mod number;

pub use derive::{DerivationName, PLATFORM_NAMESPACE, derive_id};
pub use id::{IdError, VersionRule};
pub use number::{CheckDigit, NumberError, NumberFormat, SequenceState, ShortCode, luhn_mod_36};

#[doc(hidden)]
pub mod __id_support {
    pub use crate::id::{check_version, parse_canonical};
}

#[doc(hidden)]
pub mod __private {
    pub use serde;
    pub use uuid;
}

define_id! {
    /// The id of a FHIR resource or internal record: a UUIDv7 minted by the creating node, or a
    /// UUIDv5 for a derived record.
    ResourceId, VersionRule::V7OrV5
}

define_id! {
    /// A node (central, Facility Edge Node or desktop), minted by central at provisioning.
    NodeId, VersionRule::V7
}

define_id! {
    /// An enrolled device, minted by the edge at enrollment.
    DeviceId, VersionRule::V7
}

define_id! {
    /// A sync operation, minted in the same transaction as the change it carries. It is also the
    /// end-to-end `Idempotency-Key`.
    OpId, VersionRule::V7
}
