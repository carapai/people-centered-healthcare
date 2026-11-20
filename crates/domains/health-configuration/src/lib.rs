//! Configuration: Published Definitions and their packages (configuration.md; ADRs 0024–0026).
//!
//! - [`package`]: what a FHIR NPM package of Published Definitions holds, and the rules it must
//!   meet to be published: R5 only, every dependency pinned to an exact version, a manifest that
//!   pins every canonical it depends on, and nothing outside the package layout.
//! - [`lifecycle`]: the publication lifecycle at central (ADR 0025): Draft, Validation,
//!   Simulation, Review, Approval and Publication, with separation of duties and the semver diff
//!   check.
//! - [`activation`]: scoped activation on nodes: the activation record, the active set merged
//!   along a node's scope chain, the checks before a switch, the switch itself (whole or not at
//!   all) and the versions a node retains.
//! - [`language`]: Language Packs (ADR 0066): what a pack translates, its entries keyed by a
//!   stable address with the English each translates, and which are stale or gone.
//! - [`rollout`]: staged software roll-out: which platform release each node may install, set
//!   by scope like activations, with the release's artifacts and their digests and signatures.
//!
//! Reading archives, hashing and signatures are adapters' work (`health-packages`); this crate
//! sees a package's parsed contents only.

pub mod activation;
pub mod language;
pub mod lifecycle;
pub mod package;
pub mod rollout;
