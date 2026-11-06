//! Patient engagement Module (ADR 0029). Patient access starts here as [`registration`], the
//! self-registration Registration Request (patient-identity.md § Self-registration, §59), and
//! patients' [`requests`]: proposals that only staff turn into orders (§60).

pub mod registration;
pub mod requests;
