//! Configuration: Published Definitions and their packages (configuration.md; ADRs 0024–0026).
//!
//! - [`package`]: what a FHIR NPM package of Published Definitions holds, and the rules it must
//!   meet to be published: R5 only, every dependency pinned to an exact version, a manifest that
//!   pins every canonical it depends on, and nothing outside the package layout.
//!
//! Reading archives, hashing and signatures are adapters' work (`health-packages`); this crate
//! sees a package's parsed contents only.

pub mod package;
