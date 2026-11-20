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
//! - **Drafts stay at central.** A draft has one **Draft Holder** at a time (studio.md § The
//!   authoring API; #493): only the holder edits, checks, submits, hands over or withdraws it.
//!   - A handover takes effect when the person offered it accepts; until then the holder keeps
//!     the draft. Either of them may call the offer off.
//!   - A person with `metadata.manage` reassigns a draft whose holder is unavailable, giving a
//!     reason, which the record carries.
//!   - Everyone who has held the draft is one of its [`Lifecycle::authors`].
//!   - Withdrawal keeps the draft and its history, and ends it: its `name@version` may be
//!     drafted again.
//!
//!   - The draft has one discussion: its records, and the comments anyone in the Studio adds
//!     ([`Lifecycle::comment`]), which move nothing. Records are only ever appended.
//!
//!   Until submission a draft may change version, and *Check now* records the checks' latest
//!   results without moving it.
//! - **Content is pinned.** Submitting fixes the content's fingerprint (the sha-256 of its files).
//!   Validation, simulation, review and approval all apply to that content. A revision is only
//!   possible in Draft, and a return to Draft clears the review and every approval.
//! - **Separation of duties:**
//!   - no author, which is everyone who has held the draft, reviews or approves;
//!   - clinical content (national core, terminology, program packages) needs two distinct
//!     approvers, and configuration (country configuration, overrides) needs one;
//!   - a Language Pack needs one, by someone who never held its draft, who states in the
//!     discussion that they read the language (ADR 0066).
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
    /// Withdrawn by its holder before publication: kept with its history, and ended.
    Withdrawn,
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
            Self::Withdrawn => "withdrawn",
        })
    }
}

/// How many distinct approvers a kind of package needs (ADR 0025).
pub fn approvals_required(kind: PackageKind) -> usize {
    match kind {
        PackageKind::NationalCore | PackageKind::Terminology | PackageKind::Program => 2,
        PackageKind::CountryConfiguration | PackageKind::Override | PackageKind::Language => 1,
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
    /// *Check now* ran validation and simulation on the draft, without changing its stage
    /// (studio.md § The authoring API, Checks): the latest results, not a decision.
    Checked {
        /// Each problem validation found.
        validation: Vec<String>,
        /// Each failing `tests/` case.
        simulation: Vec<String>,
    },
    /// The draft took another version, before submission (studio.md § The authoring API,
    /// Starting). The record carries the new one.
    VersionChanged {
        /// The version it had.
        previous: Version,
    },
    /// The holder offered the draft to `to`; they hold it once they accept.
    HandoverOffered {
        /// Who it is offered to (kept as `offered-to`: a record's `to` is its stage).
        #[serde(rename = "offered-to")]
        to: String,
    },
    /// The offer to `to` was called off, by the holder or by `to`.
    HandoverDeclined {
        /// Who it was offered to.
        #[serde(rename = "offered-to")]
        to: String,
    },
    /// The person offered the draft accepted it: they hold it now.
    HandedOver {
        /// Who held it before (kept as `previous-holder`: a record's `from` is its stage).
        #[serde(rename = "previous-holder")]
        from: String,
    },
    /// A manager gave the draft to `to`, whose holder was unavailable, saying why.
    Reassigned {
        /// Who held it before.
        #[serde(rename = "previous-holder")]
        from: String,
        /// Who holds it now.
        #[serde(rename = "holder")]
        to: String,
        /// Why.
        reason: String,
    },
    /// The holder withdrew it before publication.
    Withdrawn,
    /// Someone commented in the draft's discussion (studio.md § The authoring API, Discussion;
    /// #496). It moves nothing: the record keeps the stage it was at.
    Commented {
        /// What they wrote.
        text: String,
        /// The path of the artefact it is about, if it names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        about: Option<String>,
    },
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
    /// Someone who does not hold the draft tried to edit, check, submit, hand over or withdraw
    /// it.
    #[error("only the draft's holder may do that")]
    NotHolder,
    /// Someone accepted a handover not offered to them.
    #[error("the draft is not being handed over to you")]
    NotOffered,
    /// The draft was handed over or reassigned to the person who holds it.
    #[error("{0} already holds the draft")]
    AlreadyHolder(String),
}

/// The actor recorded for the automated steps.
pub const SYSTEM: &str = "system";

/// A step a person takes (studio.md § The authoring API, Allowed actions). The automated steps
/// (validation and simulation) are not among them: submission runs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// An author submits the draft.
    Submit,
    /// A reviewer accepts it.
    Review,
    /// A reviewer or an approver sends it back to Draft.
    RequestChanges,
    /// An approver approves it.
    Approve,
    /// It is signed and published.
    Publish,
    /// The holder changes the draft's content or version.
    Edit,
    /// The holder runs *Check now* on the draft.
    Check,
    /// The holder offers the draft to someone else.
    HandOver,
    /// The person offered the draft accepts it.
    AcceptHandover,
    /// The holder, or the person offered it, calls a handover off.
    DeclineHandover,
    /// A manager gives the draft to someone else.
    Reassign,
    /// The holder withdraws it.
    Withdraw,
}

impl Action {
    /// Every action, in the order a package meets them.
    pub const ALL: [Self; 12] = [
        Self::Edit,
        Self::Check,
        Self::HandOver,
        Self::AcceptHandover,
        Self::DeclineHandover,
        Self::Reassign,
        Self::Submit,
        Self::Review,
        Self::RequestChanges,
        Self::Approve,
        Self::Publish,
        Self::Withdraw,
    ];
}

/// One package version on its way to publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Stored")]
pub struct Lifecycle {
    /// The package name.
    pub package: String,
    /// The package version.
    pub version: Version,
    /// Which kind of package: it decides the approvals needed.
    pub kind: PackageKind,
    /// Where it stands.
    pub stage: Stage,
    /// Its Draft Holder: the one person who may edit, check, submit, hand over or withdraw it.
    pub holder: String,
    /// Who it is being handed over to, until they accept or the offer is called off.
    pub offered_to: Option<String>,
    /// Everyone who has held it: none of them may review or approve it (ADR 0025).
    pub authors: BTreeSet<String>,
    /// The fingerprint of its current content.
    pub content: String,
    /// Who accepted it at review, until it returns to Draft.
    pub reviewer: Option<String>,
    /// Who approved it, until it returns to Draft.
    pub approvers: BTreeSet<String>,
    /// When its holder withdrew it.
    pub withdrawn_at: Option<Timestamp>,
    /// Its holder's one change note for the whole draft, shown in *What changed* above the
    /// generated summaries (studio.md § The authoring API).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A lifecycle as it is stored, including one kept before there was a Draft Holder (#493),
/// whose holder is then its author.
#[derive(Deserialize)]
struct Stored {
    package: String,
    version: Version,
    kind: PackageKind,
    stage: Stage,
    #[serde(default)]
    holder: Option<String>,
    #[serde(default)]
    offered_to: Option<String>,
    authors: BTreeSet<String>,
    content: String,
    reviewer: Option<String>,
    approvers: BTreeSet<String>,
    #[serde(default)]
    withdrawn_at: Option<Timestamp>,
    #[serde(default)]
    note: Option<String>,
}

impl From<Stored> for Lifecycle {
    fn from(stored: Stored) -> Self {
        let holder = stored
            .holder
            .or_else(|| stored.authors.iter().next().cloned())
            .unwrap_or_default();
        Self {
            package: stored.package,
            version: stored.version,
            kind: stored.kind,
            stage: stored.stage,
            holder,
            offered_to: stored.offered_to,
            authors: stored.authors,
            content: stored.content,
            reviewer: stored.reviewer,
            approvers: stored.approvers,
            withdrawn_at: stored.withdrawn_at,
            note: stored.note,
        }
    }
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
            holder: by.to_owned(),
            offered_to: None,
            authors: BTreeSet::from([by.to_owned()]),
            content: content.to_owned(),
            reviewer: None,
            approvers: BTreeSet::new(),
            withdrawn_at: None,
            note: None,
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

    /// `by` holds the draft.
    fn holds(&self, by: &str) -> Result<(), Refused> {
        if blank(by) {
            Err(Refused::Incomplete)
        } else if self.holder == by {
            Ok(())
        } else {
            Err(Refused::NotHolder)
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

    /// Whether `by` may take `action` now, by the rules its transition enforces: the transition
    /// itself calls this, so what is offered and what is enforced cannot drift apart. A reason
    /// or a fingerprint the transition also needs is not checked here.
    pub fn check(&self, action: Action, by: &str) -> Result<(), Refused> {
        match action {
            Action::Edit | Action::Check | Action::Submit | Action::HandOver => {
                self.expect(Stage::Draft)?;
                self.holds(by)
            }
            Action::AcceptHandover => {
                self.expect(Stage::Draft)?;
                if blank(by) {
                    Err(Refused::Incomplete)
                } else if self.offered_to.as_deref() == Some(by) {
                    Ok(())
                } else {
                    Err(Refused::NotOffered)
                }
            }
            Action::DeclineHandover => {
                self.expect(Stage::Draft)?;
                if self.offered_to.as_deref() == Some(by) {
                    return Ok(());
                }
                self.holds(by)?;
                if self.offered_to.is_none() {
                    return Err(Refused::NotOffered);
                }
                Ok(())
            }
            Action::Reassign => {
                self.expect(Stage::Draft)?;
                if blank(by) {
                    return Err(Refused::Incomplete);
                }
                Ok(())
            }
            Action::Withdraw => {
                if matches!(self.stage, Stage::Published | Stage::Withdrawn) {
                    return Err(Refused::Stage(self.stage));
                }
                self.holds(by)
            }
            Action::Review => {
                self.expect(Stage::Review)?;
                self.independent(by)
            }
            Action::RequestChanges => {
                if !matches!(self.stage, Stage::Review | Stage::Approval) {
                    return Err(Refused::Stage(self.stage));
                }
                self.independent(by)
            }
            Action::Approve => {
                self.expect(Stage::Approval)?;
                self.independent(by)?;
                if self.approvers.contains(by) {
                    return Err(Refused::AlreadyApproved(by.to_owned()));
                }
                Ok(())
            }
            Action::Publish => {
                self.ready()?;
                if blank(by) {
                    return Err(Refused::Incomplete);
                }
                Ok(())
            }
        }
    }

    /// The holder, `by`, replaces the draft's content with `content`.
    pub fn revise(&mut self, by: &str, content: &str, now: Timestamp) -> Result<Record, Refused> {
        self.check(Action::Edit, by)?;
        if blank(content) {
            return Err(Refused::Incomplete);
        }
        content.clone_into(&mut self.content);
        Ok(self.record(Step::Revised, Stage::Draft, by, now))
    }

    /// The holder, `by`, gives the draft `version`, its content now fingerprinted `content`.
    /// Only a draft changes version; submission's semver check confirms it.
    pub fn change_version(
        &mut self,
        by: &str,
        version: Version,
        content: &str,
        now: Timestamp,
    ) -> Result<Record, Refused> {
        self.check(Action::Edit, by)?;
        if blank(content) {
            return Err(Refused::Incomplete);
        }
        let previous = std::mem::replace(&mut self.version, version);
        content.clone_into(&mut self.content);
        Ok(self.record(Step::VersionChanged { previous }, Stage::Draft, by, now))
    }

    /// The holder, `by`, gives the draft its one change note, or clears it with an empty one.
    /// The note is not content: it changes neither the fingerprint nor the history.
    pub fn set_note(&mut self, by: &str, note: Option<String>) -> Result<(), Refused> {
        self.check(Action::Edit, by)?;
        self.note = note
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty());
        Ok(())
    }

    /// *Check now* by `by`: the draft's validation problems and simulation failures, kept as the
    /// latest results. The draft stays where it is; submission runs the checks again, and that
    /// run decides.
    pub fn checked(
        &self,
        by: &str,
        validation: Vec<String>,
        simulation: Vec<String>,
        now: Timestamp,
    ) -> Result<Record, Refused> {
        self.check(Action::Check, by)?;
        Ok(Record {
            package: self.package.clone(),
            version: self.version.clone(),
            step: Step::Checked {
                validation,
                simulation,
            },
            from: self.stage,
            to: self.stage,
            by: by.to_owned(),
            at: now,
            content: self.content.clone(),
        })
    }

    /// `by` comments `text` in the draft's one discussion, naming the artefact at path `about`
    /// if they like. Anyone may, the authors too, until the draft is published or withdrawn.
    /// The draft stays where it is, with its review and approvals: the record is the history's,
    /// written here, never by a client. Whether `about` is one of the draft's files is the
    /// caller's to check.
    pub fn comment(
        &self,
        by: &str,
        text: &str,
        about: Option<&str>,
        now: Timestamp,
    ) -> Result<Record, Refused> {
        if matches!(self.stage, Stage::Published | Stage::Withdrawn) {
            return Err(Refused::Stage(self.stage));
        }
        if blank(by) || blank(text) {
            return Err(Refused::Incomplete);
        }
        let about = about
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_owned);
        Ok(Record {
            package: self.package.clone(),
            version: self.version.clone(),
            step: Step::Commented {
                text: text.trim().to_owned(),
                about,
            },
            from: self.stage,
            to: self.stage,
            by: by.to_owned(),
            at: now,
            content: self.content.clone(),
        })
    }

    /// The holder submits the draft for validation.
    pub fn submit(&mut self, by: &str, now: Timestamp) -> Result<Record, Refused> {
        self.check(Action::Submit, by)?;
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
        self.check(Action::Review, by)?;
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
        self.check(Action::RequestChanges, by)?;
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
        self.check(Action::Approve, by)?;
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

    /// The holder, `by`, offers the draft to `to`. They hold it once they accept; until then
    /// `by` does. A second offer replaces the first.
    pub fn hand_over(&mut self, by: &str, to: &str, now: Timestamp) -> Result<Record, Refused> {
        self.check(Action::HandOver, by)?;
        let to = to.trim();
        if blank(to) {
            return Err(Refused::Incomplete);
        }
        if to == self.holder {
            return Err(Refused::AlreadyHolder(to.to_owned()));
        }
        self.offered_to = Some(to.to_owned());
        let step = Step::HandoverOffered { to: to.to_owned() };
        Ok(self.record(step, Stage::Draft, by, now))
    }

    /// The person offered the draft, `by`, accepts it: they hold it, and are one of its
    /// authors from now on.
    pub fn accept_handover(&mut self, by: &str, now: Timestamp) -> Result<Record, Refused> {
        self.check(Action::AcceptHandover, by)?;
        let from = std::mem::replace(&mut self.holder, by.to_owned());
        self.authors.insert(by.to_owned());
        self.offered_to = None;
        Ok(self.record(Step::HandedOver { from }, Stage::Draft, by, now))
    }

    /// The holder, or the person offered the draft, calls the handover off: the holder keeps
    /// it.
    pub fn decline_handover(&mut self, by: &str, now: Timestamp) -> Result<Record, Refused> {
        self.check(Action::DeclineHandover, by)?;
        let to = self.offered_to.take().unwrap_or_default();
        Ok(self.record(Step::HandoverDeclined { to }, Stage::Draft, by, now))
    }

    /// A manager, `by`, gives the draft to `to` because its holder is unavailable, saying why.
    /// `to` holds it at once, and is one of its authors from now on. The permission is the
    /// caller's to check.
    pub fn reassign(
        &mut self,
        by: &str,
        to: &str,
        reason: &str,
        now: Timestamp,
    ) -> Result<Record, Refused> {
        self.check(Action::Reassign, by)?;
        let to = to.trim();
        if blank(to) || blank(reason) {
            return Err(Refused::Incomplete);
        }
        if to == self.holder {
            return Err(Refused::AlreadyHolder(to.to_owned()));
        }
        let from = std::mem::replace(&mut self.holder, to.to_owned());
        self.authors.insert(to.to_owned());
        self.offered_to = None;
        let step = Step::Reassigned {
            from,
            to: to.to_owned(),
            reason: reason.trim().to_owned(),
        };
        Ok(self.record(step, Stage::Draft, by, now))
    }

    /// The holder, `by`, withdraws it before publication. It is kept, with its history, and
    /// nothing further happens to it; its `name@version` may be drafted again.
    pub fn withdraw(&mut self, by: &str, now: Timestamp) -> Result<Record, Refused> {
        self.check(Action::Withdraw, by)?;
        self.offered_to = None;
        self.withdrawn_at = Some(now);
        Ok(self.record(Step::Withdrawn, Stage::Withdrawn, by, now))
    }

    /// `by` published it, signing content fingerprinted `signed`, which must be what was
    /// approved.
    pub fn publish(&mut self, by: &str, signed: &str, now: Timestamp) -> Result<Record, Refused> {
        self.check(Action::Publish, by)?;
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
