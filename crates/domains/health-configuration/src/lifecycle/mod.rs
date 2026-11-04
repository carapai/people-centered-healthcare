//! The publication lifecycle at central (configuration.md § Lifecycle; ADR 0025).
//!
//! ```text
//! Draft ──submit──▶ Validation ──passes──▶ Simulation ──passes──▶ Review ──accept──▶ Approval ──publish──▶ Published
//!   ▲                   │ fails                │ fails               │ changes            │ changes
//!   └───────────────────┴──────────────────────┴─────────────────────┴────────────────────┘
//! ```
//!
//! A [`Lifecycle`] is one package version on its way to publication. Every transition returns a
//! [`Record`]: the configuration context keeps them, and audits each one.
//!
//! - **Drafts stay at central.** Authors revise the draft; whoever revises it becomes an author.
//! - **Content is pinned.** Submitting fixes the content's fingerprint (the sha-256 of its files).
//!   Validation, simulation, review and approval all apply to that content. A revision is only
//!   possible in Draft, and a return to Draft clears the review and every approval.
//! - **Separation of duties:**
//!   - no author reviews or approves;
//!   - clinical content (national core, terminology, program packages) needs two distinct
//!     approvers, and configuration (country configuration, overrides) needs one.
//! - **Publication** needs every approval, and the fingerprint of what was signed must be the one
//!   approved. A published version is immutable: nothing further happens to it here. Activation
//!   is a separate, scoped record (#89).
//!
//! [`semver`] is the bump check validation runs against the previous published version.

pub mod semver;

use std::collections::BTreeSet;
use std::fmt;

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::package::{PackageKind, Version};

/// Where a package version stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Being authored. The only stage whose content may change.
    Draft,
    /// Submitted, waiting for the automated checks.
    Validation,
    /// Validated, waiting for its `tests/` to run in the sandbox.
    Simulation,
    /// Waiting for a reviewer who is not an author.
    Review,
    /// Reviewed, collecting approvals.
    Approval,
    /// Signed and immutable.
    Published,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Draft => "draft",
            Self::Validation => "validation",
            Self::Simulation => "simulation",
            Self::Review => "review",
            Self::Approval => "approval",
            Self::Published => "published",
        })
    }
}

/// How many distinct approvers a kind of package needs (ADR 0025).
pub fn approvals_required(kind: PackageKind) -> usize {
    match kind {
        PackageKind::NationalCore | PackageKind::Terminology | PackageKind::Program => 2,
        PackageKind::CountryConfiguration | PackageKind::Override => 1,
    }
}

/// Whether a kind of package is clinical content.
pub fn is_clinical(kind: PackageKind) -> bool {
    approvals_required(kind) > 1
}

/// What a transition did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "step")]
pub enum Step {
    /// The draft was started.
    Created,
    /// The draft's content changed.
    Revised,
    /// The author submitted it for validation.
    Submitted,
    /// The automated checks passed.
    Validated,
    /// The automated checks failed, with why.
    ValidationFailed {
        /// Each problem found.
        problems: Vec<String>,
    },
    /// The `tests/` cases passed in the sandbox.
    Simulated,
    /// A `tests/` case failed, with why.
    SimulationFailed {
        /// Each failure.
        problems: Vec<String>,
    },
    /// A reviewer accepted it.
    Reviewed,
    /// A reviewer or an approver sent it back to Draft.
    ChangesRequested {
        /// What needs to change.
        reason: String,
    },
    /// One approver approved it.
    Approved,
    /// It was signed and published.
    Published,
}

/// One transition, as the configuration context keeps and audits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The package name.
    pub package: String,
    /// The package version.
    pub version: Version,
    /// What happened.
    #[serde(flatten)]
    pub step: Step,
    /// The stage before.
    pub from: Stage,
    /// The stage after.
    pub to: Stage,
    /// Who did it: a user, or `system` for the automated steps.
    pub by: String,
    /// When.
    pub at: Timestamp,
    /// The content's fingerprint at that moment.
    pub content: String,
}

/// Why a transition was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// Not possible from the current stage.
    #[error("not possible at the {0} stage")]
    Stage(Stage),
    /// Who acted, a reason or a fingerprint was missing.
    #[error("who acted, and why where a reason is needed, must be given")]
    Incomplete,
    /// An author tried to review or approve.
    #[error("an author may not review or approve their own package")]
    Author,
    /// The same person approved twice.
    #[error("{0} has already approved this package")]
    AlreadyApproved(String),
    /// Publication before every approval.
    #[error("{have} of {need} approvals")]
    NotApproved {
        /// Approvals so far.
        have: usize,
        /// Approvals needed.
        need: usize,
    },
    /// What was signed is not what was approved.
    #[error("the signed content {signed} is not the approved content {approved}")]
    ContentChanged {
        /// The approved fingerprint.
        approved: String,
        /// The signed fingerprint.
        signed: String,
    },
    /// Someone who is not an author submitted the draft.
    #[error("only an author may submit the draft")]
    NotAuthor,
}

/// The actor recorded for the automated steps.
pub const SYSTEM: &str = "system";

/// One package version on its way to publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lifecycle {
    /// The package name.
    pub package: String,
    /// The package version.
    pub version: Version,
    /// Which kind of package: it decides the approvals needed.
    pub kind: PackageKind,
    /// Where it stands.
    pub stage: Stage,
    /// Everyone who created or revised it.
    pub authors: BTreeSet<String>,
    /// The fingerprint of its current content.
    pub content: String,
    /// Who accepted it at review, until it returns to Draft.
    pub reviewer: Option<String>,
    /// Who approved it, until it returns to Draft.
    pub approvers: BTreeSet<String>,
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

impl Lifecycle {
    /// A new draft of `package@version`, by `by`, with content fingerprinted `content`.
    pub fn create(
        package: &str,
        version: Version,
        kind: PackageKind,
        by: &str,
        content: &str,
        now: Timestamp,
    ) -> Result<(Self, Record), Refused> {
        if blank(package) || blank(by) || blank(content) {
            return Err(Refused::Incomplete);
        }
        let mut lifecycle = Self {
            package: package.to_owned(),
            version,
            kind,
            stage: Stage::Draft,
            authors: BTreeSet::from([by.to_owned()]),
            content: content.to_owned(),
            reviewer: None,
            approvers: BTreeSet::new(),
        };
        let record = lifecycle.record(Step::Created, Stage::Draft, by, now);
        Ok((lifecycle, record))
    }

    /// How many approvals publication needs.
    pub fn approvals_required(&self) -> usize {
        approvals_required(self.kind)
    }

    fn record(&mut self, step: Step, to: Stage, by: &str, at: Timestamp) -> Record {
        let from = self.stage;
        self.stage = to;
        if to == Stage::Draft {
            self.reviewer = None;
            self.approvers.clear();
        }
        Record {
            package: self.package.clone(),
            version: self.version.clone(),
            step,
            from,
            to,
            by: by.to_owned(),
            at,
            content: self.content.clone(),
        }
    }

    fn expect(&self, stage: Stage) -> Result<(), Refused> {
        if self.stage == stage {
            Ok(())
        } else {
            Err(Refused::Stage(self.stage))
        }
    }

    fn independent(&self, by: &str) -> Result<(), Refused> {
        if blank(by) {
            Err(Refused::Incomplete)
        } else if self.authors.contains(by) {
            Err(Refused::Author)
        } else {
            Ok(())
        }
    }

    /// `by` replaces the draft's content with `content`, and becomes an author.
    pub fn revise(&mut self, by: &str, content: &str, now: Timestamp) -> Result<Record, Refused> {
        self.expect(Stage::Draft)?;
        if blank(by) || blank(content) {
            return Err(Refused::Incomplete);
        }
        self.authors.insert(by.to_owned());
        content.clone_into(&mut self.content);
        Ok(self.record(Step::Revised, Stage::Draft, by, now))
    }

    /// An author submits the draft for validation.
    pub fn submit(&mut self, by: &str, now: Timestamp) -> Result<Record, Refused> {
        self.expect(Stage::Draft)?;
        if !self.authors.contains(by) {
            return Err(Refused::NotAuthor);
        }
        Ok(self.record(Step::Submitted, Stage::Validation, by, now))
    }

    /// The automated checks' outcome: no problems moves it to Simulation, any problem back to
    /// Draft.
    pub fn validated(&mut self, problems: Vec<String>, now: Timestamp) -> Result<Record, Refused> {
        self.expect(Stage::Validation)?;
        Ok(if problems.is_empty() {
            self.record(Step::Validated, Stage::Simulation, SYSTEM, now)
        } else {
            self.record(
                Step::ValidationFailed { problems },
                Stage::Draft,
                SYSTEM,
                now,
            )
        })
    }

    /// The sandbox's outcome: no failure moves it to Review, any failure back to Draft.
    pub fn simulated(&mut self, problems: Vec<String>, now: Timestamp) -> Result<Record, Refused> {
        self.expect(Stage::Simulation)?;
        Ok(if problems.is_empty() {
            self.record(Step::Simulated, Stage::Review, SYSTEM, now)
        } else {
            self.record(
                Step::SimulationFailed { problems },
                Stage::Draft,
                SYSTEM,
                now,
            )
        })
    }

    /// A reviewer who is not an author accepts it.
    pub fn review(&mut self, by: &str, now: Timestamp) -> Result<Record, Refused> {
        self.expect(Stage::Review)?;
        self.independent(by)?;
        self.reviewer = Some(by.to_owned());
        Ok(self.record(Step::Reviewed, Stage::Approval, by, now))
    }

    /// A reviewer or an approver, not an author, sends it back to Draft, saying why.
    pub fn request_changes(
        &mut self,
        by: &str,
        reason: &str,
        now: Timestamp,
    ) -> Result<Record, Refused> {
        if !matches!(self.stage, Stage::Review | Stage::Approval) {
            return Err(Refused::Stage(self.stage));
        }
        self.independent(by)?;
        if blank(reason) {
            return Err(Refused::Incomplete);
        }
        let step = Step::ChangesRequested {
            reason: reason.to_owned(),
        };
        Ok(self.record(step, Stage::Draft, by, now))
    }

    /// An approver who is not an author, and has not approved it yet, approves it.
    pub fn approve(&mut self, by: &str, now: Timestamp) -> Result<Record, Refused> {
        self.expect(Stage::Approval)?;
        self.independent(by)?;
        if self.approvers.contains(by) {
            return Err(Refused::AlreadyApproved(by.to_owned()));
        }
        self.approvers.insert(by.to_owned());
        Ok(self.record(Step::Approved, Stage::Approval, by, now))
    }

    /// Whether publication may go ahead: every approval is in.
    pub fn ready(&self) -> Result<(), Refused> {
        self.expect(Stage::Approval)?;
        let need = self.approvals_required();
        let have = self.approvers.len();
        if have < need {
            return Err(Refused::NotApproved { have, need });
        }
        Ok(())
    }

    /// `by` published it, signing content fingerprinted `signed`, which must be what was
    /// approved.
    pub fn publish(&mut self, by: &str, signed: &str, now: Timestamp) -> Result<Record, Refused> {
        self.ready()?;
        if blank(by) {
            return Err(Refused::Incomplete);
        }
        if signed != self.content {
            return Err(Refused::ContentChanged {
                approved: self.content.clone(),
                signed: signed.to_owned(),
            });
        }
        Ok(self.record(Step::Published, Stage::Published, by, now))
    }
}

#[cfg(test)]
mod tests;
