//! A node's identity on its host, shared by the edge and the desktop and composed in their
//! `apps/` roots (dependency-rules.md rule 4).
//!
//! - [`encryption`]: the full-disk encryption check, the provisioning gate and its exemption
//!   (ADR 0048).
//!
//! Node keys in the OS keystore and the facility intermediate's installation follow (#73).

pub mod encryption;
