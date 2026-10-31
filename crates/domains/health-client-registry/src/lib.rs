//! Client registry Module: source Patients, golden records and their links (ADR 0027;
//! patient-identity.md § Golden record model), and patient matching ([`matching`]; ADRs 0028,
//! 0055).
//!
//! - **Source Patients** keep their own ids; clinical records reference them, never the golden
//!   record.
//! - **The golden record** carries the ECID, links to every source (`seealso`, with `refer` back),
//!   and holds each demographic field's surviving value by trust ([`survive`]).
//! - **Unlinking** splits a source off and leaves an adjudication marker, so matching never
//!   re-links the pair. Nothing is merged automatically.
//! - **`$merge`** is only for two source records of the same facility, after a preview and a
//!   human approval ([`plan_merge`]); references are never rewritten.

mod ecid;
mod golden;
pub mod matching;
mod merge;
mod survivorship;

pub use ecid::{Ecid, InvalidEcid};
pub use golden::{GoldenRecord, LinkRefused, SourceRecord};
pub use merge::{MergePlan, MergeRefused, plan_merge};
pub use survivorship::{Claim, Field, ReviewItem, Survival, Trust, survive};
