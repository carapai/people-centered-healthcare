//! A node's identity on its host, shared by the edge and the desktop and composed in their
//! `apps/` roots (dependency-rules.md rule 4).
//!
//! - [`encryption`]: the full-disk encryption check, the provisioning gate and its exemption
//!   (ADR 0048).
//! - [`keystore`]: sealed secrets, such as the facility intermediate's key: systemd-creds on
//!   Linux, DPAPI in machine scope on Windows.

pub mod encryption;
pub mod keystore;
