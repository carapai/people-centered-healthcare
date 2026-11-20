//! The Studio API's documented bodies (`utoipa`), from what `health_publication::studio` gives.

use health_configuration::lifecycle::semver::Bump;
use health_configuration::lifecycle::{Record, Stage, Step};
use health_configuration::package::PackageKind;
use health_publication::studio::{
    ActiveIn, Allowed, ArtefactChange, ArtefactChangeKind, CheckKind, CheckResult, PackageFile,
    PackagePage, PackageSummary, Refusal, RefusalCode, StudioAction,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A step the Studio offers on a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioActionName {
    Submit,
    Review,
    Approve,
    Publish,
    Activate,
    RequestChanges,
    Check,
    AcceptHandover,
    HandOver,
    DeclineHandover,
    Withdraw,
    Reassign,
}

impl StudioActionName {
    /// The step named `text` in a path.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(text.to_owned())).ok()
    }
}

impl From<StudioAction> for StudioActionName {
    fn from(action: StudioAction) -> Self {
        match action {
            StudioAction::Submit => Self::Submit,
            StudioAction::Review => Self::Review,
            StudioAction::Approve => Self::Approve,
            StudioAction::Publish => Self::Publish,
            StudioAction::Activate => Self::Activate,
            StudioAction::RequestChanges => Self::RequestChanges,
            StudioAction::Check => Self::Check,
            StudioAction::AcceptHandover => Self::AcceptHandover,
            StudioAction::HandOver => Self::HandOver,
            StudioAction::DeclineHandover => Self::DeclineHandover,
            StudioAction::Withdraw => Self::Withdraw,
            StudioAction::Reassign => Self::Reassign,
        }
    }
}

impl From<StudioActionName> for StudioAction {
    fn from(action: StudioActionName) -> Self {
        match action {
            StudioActionName::Submit => Self::Submit,
            StudioActionName::Review => Self::Review,
            StudioActionName::Approve => Self::Approve,
            StudioActionName::Publish => Self::Publish,
            StudioActionName::Activate => Self::Activate,
            StudioActionName::RequestChanges => Self::RequestChanges,
            StudioActionName::Check => Self::Check,
            StudioActionName::AcceptHandover => Self::AcceptHandover,
            StudioActionName::HandOver => Self::HandOver,
            StudioActionName::DeclineHandover => Self::DeclineHandover,
            StudioActionName::Withdraw => Self::Withdraw,
            StudioActionName::Reassign => Self::Reassign,
        }
    }
}

/// Where a package version stands in the lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PackageStage {
    Draft,
    Validation,
    Simulation,
    Review,
    Approval,
    Published,
    Withdrawn,
}

impl From<Stage> for PackageStage {
    fn from(stage: Stage) -> Self {
        match stage {
            Stage::Draft => Self::Draft,
            Stage::Validation => Self::Validation,
            Stage::Simulation => Self::Simulation,
            Stage::Review => Self::Review,
            Stage::Approval => Self::Approval,
            Stage::Published => Self::Published,
            Stage::Withdrawn => Self::Withdrawn,
        }
    }
}

/// The kind of package, which decides its approvers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioPackageKind {
    NationalCore,
    Terminology,
    Program,
    CountryConfiguration,
    Override,
    Language,
}

impl From<StudioPackageKind> for PackageKind {
    fn from(kind: StudioPackageKind) -> Self {
        match kind {
            StudioPackageKind::NationalCore => Self::NationalCore,
            StudioPackageKind::Terminology => Self::Terminology,
            StudioPackageKind::Program => Self::Program,
            StudioPackageKind::CountryConfiguration => Self::CountryConfiguration,
            StudioPackageKind::Override => Self::Override,
            StudioPackageKind::Language => Self::Language,
        }
    }
}

impl From<PackageKind> for StudioPackageKind {
    fn from(kind: PackageKind) -> Self {
        match kind {
            PackageKind::NationalCore => Self::NationalCore,
            PackageKind::Terminology => Self::Terminology,
            PackageKind::Program => Self::Program,
            PackageKind::CountryConfiguration => Self::CountryConfiguration,
            PackageKind::Override => Self::Override,
            PackageKind::Language => Self::Language,
        }
    }
}

/// How far the version moved from the previous published one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SemverBump {
    None,
    Patch,
    Minor,
    Major,
}

impl From<Bump> for SemverBump {
    fn from(bump: Bump) -> Self {
        match bump {
            Bump::None => Self::None,
            Bump::Patch => Self::Patch,
            Bump::Minor => Self::Minor,
            Bump::Major => Self::Major,
        }
    }
}

/// What kind of refusal: the client words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioRefusalCode {
    Permission,
    Stage,
    Author,
    NotHolder,
    NotOffered,
    AlreadyApproved,
    ApprovalsNeeded,
    NotPublished,
    Other,
}

/// Why a step is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionRefusal {
    /// What kind of refusal.
    pub(crate) code: StudioRefusalCode,
    /// In English, e.g. "You wrote this draft, so you cannot review or approve it".
    pub(crate) message: String,
    /// The permission missing, for `permission`.
    pub(crate) permission: Option<String>,
    /// The package's stage, for `stage`.
    pub(crate) stage: Option<PackageStage>,
    /// How many more approvals, for `approvals-needed`.
    pub(crate) missing: Option<u32>,
}

impl From<&Refusal> for ActionRefusal {
    fn from(refusal: &Refusal) -> Self {
        Self {
            code: match refusal.code {
                RefusalCode::Permission => StudioRefusalCode::Permission,
                RefusalCode::Stage => StudioRefusalCode::Stage,
                RefusalCode::Author => StudioRefusalCode::Author,
                RefusalCode::NotHolder => StudioRefusalCode::NotHolder,
                RefusalCode::NotOffered => StudioRefusalCode::NotOffered,
                RefusalCode::AlreadyApproved => StudioRefusalCode::AlreadyApproved,
                RefusalCode::ApprovalsNeeded => StudioRefusalCode::ApprovalsNeeded,
                RefusalCode::NotPublished => StudioRefusalCode::NotPublished,
                RefusalCode::Other => StudioRefusalCode::Other,
            },
            message: refusal.message.clone(),
            permission: refusal.permission.clone(),
            stage: refusal.stage.map(PackageStage::from),
            missing: refusal
                .missing
                .map(|n| u32::try_from(n).unwrap_or(u32::MAX)),
        }
    }
}

/// One step on a package, and why the signed-in person may not take it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageAction {
    /// The step.
    pub(crate) action: StudioActionName,
    /// Whether they may take it now.
    pub(crate) allowed: bool,
    /// Why not, when they may not.
    pub(crate) refusal: Option<ActionRefusal>,
}

impl From<&Allowed> for PackageAction {
    fn from(allowed: &Allowed) -> Self {
        Self {
            action: allowed.action.into(),
            allowed: allowed.refused.is_none(),
            refusal: allowed.refused.as_ref().map(ActionRefusal::from),
        }
    }
}

/// A version active in a scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveScope {
    /// The scope, in words (e.g. `national`, `district Kampala`).
    pub(crate) scope: String,
    /// The version active there.
    pub(crate) version: String,
}

impl From<&ActiveIn> for ActiveScope {
    fn from(active: &ActiveIn) -> Self {
        Self {
            scope: active.scope.to_string(),
            version: active.version.to_string(),
        }
    }
}

/// One row of the package list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioPackage {
    /// `name@version`.
    pub(crate) id: String,
    /// The package name.
    pub(crate) name: String,
    /// Its title, from its manifest.
    pub(crate) title: Option<String>,
    /// Its kind.
    pub(crate) kind: StudioPackageKind,
    /// This version.
    pub(crate) version: String,
    /// The latest published version before it.
    pub(crate) previous: Option<String>,
    /// How far the version moved from `previous`; absent for a first version.
    pub(crate) bump: Option<SemverBump>,
    /// Where it stands.
    pub(crate) stage: PackageStage,
    /// Its Draft Holder (an account id): the one person who may edit, check, submit, hand over
    /// or withdraw it.
    pub(crate) holder: String,
    /// Who it is being handed over to (an account id), until they accept or it is called off.
    pub(crate) offered_to: Option<String>,
    /// Everyone who has held it: none of them may review or approve it.
    pub(crate) authors: Vec<String>,
    /// The scopes the package is active in now.
    pub(crate) active: Vec<ActiveScope>,
    /// Every step, each with whether the signed-in person may take it and why not.
    pub(crate) actions: Vec<PackageAction>,
    /// Their next step: the first they may take; absent when there is nothing for them.
    pub(crate) next: Option<StudioActionName>,
}

impl From<&PackageSummary> for StudioPackage {
    fn from(summary: &PackageSummary) -> Self {
        Self {
            id: summary.id.clone(),
            name: summary.name.clone(),
            title: summary.title.clone(),
            kind: summary.kind.into(),
            version: summary.version.to_string(),
            previous: summary.previous.as_ref().map(ToString::to_string),
            bump: summary.bump.map(SemverBump::from),
            stage: summary.stage.into(),
            holder: summary.holder.clone(),
            offered_to: summary.offered_to.clone(),
            authors: summary.authors.clone(),
            active: summary.active.iter().map(ActiveScope::from).collect(),
            actions: summary.actions.iter().map(PackageAction::from).collect(),
            next: summary.next.map(StudioActionName::from),
        }
    }
}

/// The package list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct StudioPackageList {
    /// One row per package, by name.
    pub(crate) packages: Vec<StudioPackage>,
}

/// How an artefact changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ArtefactChangeType {
    Added,
    Changed,
    Removed,
}

/// One added, changed or removed artefact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioArtefactChange {
    /// How it changed.
    pub(crate) change: ArtefactChangeType,
    /// Its `resourceType`.
    pub(crate) resource_type: String,
    /// Its path in the package.
    pub(crate) path: String,
    /// Its canonical URL.
    pub(crate) url: Option<String>,
    /// Its version before.
    pub(crate) before: Option<String>,
    /// Its version now.
    pub(crate) after: Option<String>,
}

impl From<&ArtefactChange> for StudioArtefactChange {
    fn from(change: &ArtefactChange) -> Self {
        Self {
            change: match change.change {
                ArtefactChangeKind::Added => ArtefactChangeType::Added,
                ArtefactChangeKind::Changed => ArtefactChangeType::Changed,
                ArtefactChangeKind::Removed => ArtefactChangeType::Removed,
            },
            resource_type: change.resource_type.clone(),
            path: change.path.clone(),
            url: change.url.clone(),
            before: change.before.clone(),
            after: change.after.clone(),
        }
    }
}

/// Which automated check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioCheck {
    Validation,
    Simulation,
}

/// The latest stored result of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioCheckResult {
    /// Which check.
    pub(crate) check: StudioCheck,
    /// Whether it passed.
    pub(crate) passed: bool,
    /// Every problem found.
    pub(crate) problems: Vec<String>,
    /// When it ran (RFC 3339).
    pub(crate) at: String,
    /// Whether it ran on the current content.
    pub(crate) current: bool,
}

impl From<&CheckResult> for StudioCheckResult {
    fn from(result: &CheckResult) -> Self {
        Self {
            check: match result.check {
                CheckKind::Validation => StudioCheck::Validation,
                CheckKind::Simulation => StudioCheck::Simulation,
            },
            passed: result.passed,
            problems: result.problems.clone(),
            at: result.at.to_string(),
            current: result.current,
        }
    }
}

/// What happened in one history entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioHistoryStep {
    Created,
    Revised,
    Submitted,
    Validated,
    ValidationFailed,
    Simulated,
    SimulationFailed,
    Reviewed,
    ChangesRequested,
    Approved,
    Published,
    Checked,
    VersionChanged,
    HandoverOffered,
    HandoverDeclined,
    HandedOver,
    Reassigned,
    Withdrawn,
}

/// One entry of the discussion: a lifecycle record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioHistoryEntry {
    /// What happened.
    pub(crate) step: StudioHistoryStep,
    /// Who did it (`system` for the automated checks).
    pub(crate) by: String,
    /// When (RFC 3339).
    pub(crate) at: String,
    /// The stage after.
    pub(crate) stage: PackageStage,
    /// What a reviewer or approver asked to change, or why a manager reassigned the draft.
    pub(crate) reason: Option<String>,
    /// Who the draft was offered or reassigned to; who held it before, when it was handed over.
    pub(crate) person: Option<String>,
    /// The problems a failed check found; for *Check now*, validation's then simulation's.
    pub(crate) problems: Vec<String>,
    /// The version the draft had, when it took another.
    pub(crate) previous: Option<String>,
}

impl From<&Record> for StudioHistoryEntry {
    fn from(record: &Record) -> Self {
        let (step, reason, problems) = match &record.step {
            Step::Created => (StudioHistoryStep::Created, None, Vec::new()),
            Step::Revised => (StudioHistoryStep::Revised, None, Vec::new()),
            Step::Submitted => (StudioHistoryStep::Submitted, None, Vec::new()),
            Step::Validated => (StudioHistoryStep::Validated, None, Vec::new()),
            Step::ValidationFailed { problems } => {
                (StudioHistoryStep::ValidationFailed, None, problems.clone())
            }
            Step::Simulated => (StudioHistoryStep::Simulated, None, Vec::new()),
            Step::SimulationFailed { problems } => {
                (StudioHistoryStep::SimulationFailed, None, problems.clone())
            }
            Step::Reviewed => (StudioHistoryStep::Reviewed, None, Vec::new()),
            Step::ChangesRequested { reason } => (
                StudioHistoryStep::ChangesRequested,
                Some(reason.clone()),
                Vec::new(),
            ),
            Step::Approved => (StudioHistoryStep::Approved, None, Vec::new()),
            Step::Published => (StudioHistoryStep::Published, None, Vec::new()),
            Step::Checked {
                validation,
                simulation,
            } => (
                StudioHistoryStep::Checked,
                None,
                validation.iter().chain(simulation).cloned().collect(),
            ),
            Step::VersionChanged { .. } => (StudioHistoryStep::VersionChanged, None, Vec::new()),
            Step::HandoverOffered { .. } => (StudioHistoryStep::HandoverOffered, None, Vec::new()),
            Step::HandoverDeclined { .. } => {
                (StudioHistoryStep::HandoverDeclined, None, Vec::new())
            }
            Step::HandedOver { .. } => (StudioHistoryStep::HandedOver, None, Vec::new()),
            Step::Reassigned { reason, .. } => (
                StudioHistoryStep::Reassigned,
                Some(reason.clone()),
                Vec::new(),
            ),
            Step::Withdrawn => (StudioHistoryStep::Withdrawn, None, Vec::new()),
        };
        let person = match &record.step {
            Step::HandoverOffered { to }
            | Step::HandoverDeclined { to }
            | Step::Reassigned { to, .. } => Some(to.clone()),
            Step::HandedOver { from } => Some(from.clone()),
            _ => None,
        };
        let previous = match &record.step {
            Step::VersionChanged { previous } => Some(previous.to_string()),
            _ => None,
        };
        Self {
            step,
            by: record.by.clone(),
            at: record.at.to_string(),
            stage: record.to.into(),
            reason,
            person,
            problems,
            previous,
        }
    }
}

/// One package version read from top to bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioPackagePage {
    /// Its row in the list, with the actions.
    pub(crate) package: StudioPackage,
    /// What changed against the previous published version.
    pub(crate) changes: Vec<StudioArtefactChange>,
    /// The stored check results.
    pub(crate) checks: Vec<StudioCheckResult>,
    /// How many approvals its kind needs.
    pub(crate) approvals_required: u32,
    /// Who accepted it at review.
    pub(crate) reviewer: Option<String>,
    /// Who has approved it.
    pub(crate) approvers: Vec<String>,
    /// The history, oldest first.
    pub(crate) history: Vec<StudioHistoryEntry>,
    /// The draft's revision, also its `ETag`: a save sends it back as `If-Match`.
    pub(crate) revision: u64,
    /// Whether the signed-in person may change the draft's files and version.
    pub(crate) editable: bool,
    /// Why not, when they may not.
    pub(crate) edit_refusal: Option<ActionRefusal>,
    /// Its files, to open in an editor.
    pub(crate) files: Vec<StudioPackageFile>,
}

/// One file of a package version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioPackageFile {
    /// Its path in the package.
    pub(crate) path: String,
    /// Its `resourceType`, for a FHIR artefact.
    pub(crate) resource_type: Option<String>,
    /// Its canonical URL.
    pub(crate) url: Option<String>,
    /// Its version.
    pub(crate) version: Option<String>,
}

impl From<&PackageFile> for StudioPackageFile {
    fn from(file: &PackageFile) -> Self {
        Self {
            path: file.path.clone(),
            resource_type: file.resource_type.clone(),
            url: file.url.clone(),
            version: file.version.clone(),
        }
    }
}

/// One file of a package version, for an editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioFile {
    /// Its path in the package.
    pub(crate) path: String,
    /// Its text.
    pub(crate) content: String,
    /// The draft's revision when it was read, also the `ETag`.
    pub(crate) revision: u64,
}

/// A file's new text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioFileInput {
    /// The whole file.
    pub(crate) content: String,
}

/// A draft's new version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioVersionInput {
    /// An exact version, e.g. `2.1.0`.
    pub(crate) version: String,
}

/// How to start a draft: a copy of a published version, or a blank new package.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioNewDraft {
    /// The published `name@version` to copy; the draft takes the next minor version.
    #[serde(default)]
    pub(crate) copy_of: Option<String>,
    /// A blank package's name, e.g. `ug.moh.tb`.
    #[serde(default)]
    pub(crate) name: Option<String>,
    /// A blank package's kind.
    #[serde(default)]
    pub(crate) kind: Option<StudioPackageKind>,
    /// A blank package's title.
    #[serde(default)]
    pub(crate) title: Option<String>,
    /// A blank package's first version, e.g. `0.1.0`.
    #[serde(default)]
    pub(crate) version: Option<String>,
    /// The canonical base of a blank package's artefacts, e.g. `http://health.go.ug/fhir`.
    #[serde(default)]
    pub(crate) canonical: Option<String>,
}

impl From<&PackagePage> for StudioPackagePage {
    fn from(page: &PackagePage) -> Self {
        Self {
            package: StudioPackage::from(&page.summary),
            changes: page
                .changes
                .iter()
                .map(StudioArtefactChange::from)
                .collect(),
            checks: page.checks.iter().map(StudioCheckResult::from).collect(),
            approvals_required: u32::try_from(page.approvals_required).unwrap_or(u32::MAX),
            reviewer: page.reviewer.clone(),
            approvers: page.approvers.clone(),
            history: page.history.iter().map(StudioHistoryEntry::from).collect(),
            revision: page.revision,
            editable: page.edit.is_none(),
            edit_refusal: page.edit.as_ref().map(ActionRefusal::from),
            files: page.files.iter().map(StudioPackageFile::from).collect(),
        }
    }
}

/// What a step needs besides who takes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioActionInput {
    /// Why changes are requested (`request-changes`), why the draft is reassigned
    /// (`reassign`), or why it is activated (`activate`).
    #[serde(default)]
    pub(crate) reason: Option<String>,
    /// The username of who the draft goes to (`hand-over`, `reassign`).
    #[serde(default)]
    pub(crate) to: Option<String>,
    /// Where to activate it (`activate`): `national`, `district:<code>`, `facility:<code>` or
    /// `program:<facility>:<program>`.
    #[serde(default)]
    pub(crate) scope: Option<String>,
}

/// Why a Studio request failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct StudioProblem {
    /// What went wrong.
    pub(crate) detail: String,
    /// Why the step was refused, when it was.
    pub(crate) refusal: Option<ActionRefusal>,
}
