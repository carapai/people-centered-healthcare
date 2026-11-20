//! What the Studio shows of the lifecycle (studio.md § The package workspace, § The authoring
//! API; #492): the package list, one draft read from top to bottom, and for each package the
//! actions the signed-in person may take, with the reason for each one refused.
//!
//! The actions are derived from the rules that enforce them: [`Lifecycle::check`] is what each
//! transition calls, and the permission is the one the Studio's API checks before acting. The
//! list is advisory: every action is checked again when it is taken (ADR 0025).
//!
//! *Check now* (#494) is listed with the others, but is never anyone's next step, nor are
//! handing over, calling a handover off, withdrawing and reassigning (#493): accepting a handover
//! is. Whether the person may change the draft is [`edit_refusal`], and the page carries the
//! draft's revision and files for the editors.
//!
//! The list shows each package's newest version that is not withdrawn; a withdrawn one shows
//! only when the package has nothing else, and every withdrawn one is still read by its id.

use std::collections::BTreeMap;

use health_configuration::activation::{ActivationRecord, Scope};
use health_configuration::lifecycle::semver::{Bump, Nature};
use health_configuration::lifecycle::{Action, Lifecycle, Record, Refused, Stage, Step};
use health_configuration::package::{INDEX, MANIFEST, PackageKind, Version};
use health_packages::Packed;
use health_surrealdb::SurrealTx;
use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{Error, Publications, PublishedPackage, failed, fingerprints, lifecycle_id};

/// Start, edit, check, submit, hand over and withdraw a draft.
pub const AUTHOR_PERMISSION: &str = "metadata.author";
/// Take a review and request changes.
pub const REVIEW_PERMISSION: &str = "metadata.review";
/// Approve.
pub const APPROVE_PERMISSION: &str = "metadata.approve";
/// Publish.
pub const PUBLISH_PERMISSION: &str = "metadata.publish";
/// Activate a published version.
pub const ACTIVATE_PERMISSION: &str = "configuration.activate";
/// Reassign a draft whose holder is unavailable.
pub const MANAGE_PERMISSION: &str = "metadata.manage";

/// The permissions that open the Studio: any one of them.
pub const PERMISSIONS: [&str; 6] = [
    AUTHOR_PERMISSION,
    REVIEW_PERMISSION,
    APPROVE_PERMISSION,
    PUBLISH_PERMISSION,
    ACTIVATE_PERMISSION,
    MANAGE_PERMISSION,
];

/// An action the Studio offers on a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StudioAction {
    /// Submit the draft: validation and simulation run at once.
    Submit,
    /// Accept it at review.
    Review,
    /// Send it back to Draft, saying why.
    RequestChanges,
    /// Approve it.
    Approve,
    /// Sign and publish it.
    Publish,
    /// Activate the published version in a scope.
    Activate,
    /// *Check now*: run validation and simulation on the draft, keeping the results.
    Check,
    /// Accept a handover offered to them: they hold the draft from then on.
    AcceptHandover,
    /// Offer the draft to someone else, who holds it once they accept.
    HandOver,
    /// Call a handover off: the holder keeps the draft.
    DeclineHandover,
    /// Withdraw the draft: kept with its history, its `name@version` free again.
    Withdraw,
    /// Give a draft whose holder is unavailable to someone else, saying why.
    Reassign,
}

impl StudioAction {
    /// Every action, in the order a package meets them. Accepting a handover comes first: the
    /// draft waits for it. Requesting changes, *Check now*, handing over, calling a handover off,
    /// withdrawing and reassigning come last: none is the step a package is waiting for.
    pub const ALL: [Self; 12] = [
        Self::AcceptHandover,
        Self::Submit,
        Self::Review,
        Self::Approve,
        Self::Publish,
        Self::Activate,
        Self::RequestChanges,
        Self::Check,
        Self::HandOver,
        Self::DeclineHandover,
        Self::Withdraw,
        Self::Reassign,
    ];

    /// The permission it needs (studio.md § The authoring API, Permissions).
    pub fn permission(self) -> &'static str {
        match self {
            Self::Submit
            | Self::Check
            | Self::AcceptHandover
            | Self::HandOver
            | Self::DeclineHandover
            | Self::Withdraw => AUTHOR_PERMISSION,
            Self::Review | Self::RequestChanges => REVIEW_PERMISSION,
            Self::Approve => APPROVE_PERMISSION,
            Self::Publish => PUBLISH_PERMISSION,
            Self::Activate => ACTIVATE_PERMISSION,
            Self::Reassign => MANAGE_PERMISSION,
        }
    }

    /// Whether a package waits for it: only such an action is someone's next step.
    fn awaited(self) -> bool {
        !matches!(
            self,
            Self::Check | Self::HandOver | Self::DeclineHandover | Self::Withdraw | Self::Reassign
        )
    }

    fn lifecycle(self) -> Option<Action> {
        match self {
            Self::Submit => Some(Action::Submit),
            Self::Review => Some(Action::Review),
            Self::RequestChanges => Some(Action::RequestChanges),
            Self::Approve => Some(Action::Approve),
            Self::Publish => Some(Action::Publish),
            Self::Check => Some(Action::Check),
            Self::AcceptHandover => Some(Action::AcceptHandover),
            Self::HandOver => Some(Action::HandOver),
            Self::DeclineHandover => Some(Action::DeclineHandover),
            Self::Withdraw => Some(Action::Withdraw),
            Self::Reassign => Some(Action::Reassign),
            Self::Activate => None,
        }
    }
}

/// Why an action is refused, as a code the client words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefusalCode {
    /// The person's roles do not carry the permission.
    Permission,
    /// Not possible at the package's stage.
    Stage,
    /// They wrote the draft, so they may not review or approve it.
    Author,
    /// Only the Draft Holder may do that.
    NotHolder,
    /// No handover is offered to them (or, to the holder, none is pending).
    NotOffered,
    /// They have approved it already.
    AlreadyApproved,
    /// More approvals are needed before publication.
    ApprovalsNeeded,
    /// Only a published version can be activated.
    NotPublished,
    /// Another refusal, in words.
    Other,
}

/// Why an action is refused: a code, its words, and what the words name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// What kind of refusal.
    pub code: RefusalCode,
    /// In words, e.g. "You wrote this draft, so you cannot review or approve it".
    pub message: String,
    /// The permission missing, for [`RefusalCode::Permission`].
    pub permission: Option<String>,
    /// The package's stage, for [`RefusalCode::Stage`].
    pub stage: Option<Stage>,
    /// How many more approvals, for [`RefusalCode::ApprovalsNeeded`].
    pub missing: Option<usize>,
}

impl Refusal {
    fn new(code: RefusalCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            permission: None,
            stage: None,
            missing: None,
        }
    }

    /// The permission `permission` is missing.
    pub fn permission(permission: &str) -> Self {
        Self {
            permission: Some(permission.to_owned()),
            ..Self::new(
                RefusalCode::Permission,
                format!("You need the {permission} permission"),
            )
        }
    }

    /// Only a published version can be activated.
    pub fn not_published() -> Self {
        Self::new(
            RefusalCode::NotPublished,
            "Only a published version can be activated",
        )
    }
}

impl From<&Refused> for Refusal {
    fn from(refused: &Refused) -> Self {
        match refused {
            Refused::Stage(stage) => Self {
                stage: Some(*stage),
                ..Self::new(
                    RefusalCode::Stage,
                    format!("Not possible while it is at the {stage} stage"),
                )
            },
            Refused::Author => Self::new(
                RefusalCode::Author,
                "You wrote this draft, so you cannot review or approve it",
            ),
            Refused::NotHolder => Self::new(
                RefusalCode::NotHolder,
                "Only the draft's holder may do that",
            ),
            Refused::NotOffered => Self::new(
                RefusalCode::NotOffered,
                "No handover of this draft is waiting for you",
            ),
            Refused::AlreadyApproved(_) => Self::new(
                RefusalCode::AlreadyApproved,
                "You have already approved this draft",
            ),
            Refused::NotApproved { have, need } => {
                let missing = need.saturating_sub(*have);
                Self {
                    missing: Some(missing),
                    ..Self::new(
                        RefusalCode::ApprovalsNeeded,
                        if missing == 1 {
                            "1 more approval needed".to_owned()
                        } else {
                            format!("{missing} more approvals needed")
                        },
                    )
                }
            }
            other => Self::new(RefusalCode::Other, other.to_string()),
        }
    }
}

/// One action, and why it is refused if it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowed {
    /// The action.
    pub action: StudioAction,
    /// Why the person may not take it; `None` if they may.
    pub refused: Option<Refusal>,
}

/// Why `by`, whose roles carry the permissions `has` answers for, may not take `action` on
/// `lifecycle`; `None` if they may.
pub fn refusal(
    lifecycle: &Lifecycle,
    action: StudioAction,
    by: &str,
    has: &(dyn Fn(&str) -> bool + Sync),
) -> Option<Refusal> {
    if !has(action.permission()) {
        return Some(Refusal::permission(action.permission()));
    }
    match action.lifecycle() {
        Some(step) => lifecycle
            .check(step, by)
            .err()
            .map(|refused| Refusal::from(&refused)),
        None => (lifecycle.stage != Stage::Published).then(Refusal::not_published),
    }
}

/// Every action on `lifecycle`, each with why `by` may not take it, if they may not.
pub fn allowed(
    lifecycle: &Lifecycle,
    by: &str,
    has: &(dyn Fn(&str) -> bool + Sync),
) -> Vec<Allowed> {
    StudioAction::ALL
        .into_iter()
        .map(|action| Allowed {
            action,
            refused: refusal(lifecycle, action, by, has),
        })
        .collect()
}

/// The first action the person may take that the package waits for: their next step, or
/// nothing for them. *Check now*, handing over, calling a handover off, withdrawing and
/// reassigning never are.
pub fn next_step(actions: &[Allowed]) -> Option<StudioAction> {
    actions
        .iter()
        .find(|allowed| allowed.refused.is_none() && allowed.action.awaited())
        .map(|allowed| allowed.action)
}

/// Why `by` may not change `lifecycle`'s draft (its files or its version); `None` if they may.
/// The same rule [`Lifecycle::revise`] enforces, the Draft Holder's, behind the author
/// permission.
pub fn edit_refusal(
    lifecycle: &Lifecycle,
    by: &str,
    has: &(dyn Fn(&str) -> bool + Sync),
) -> Option<Refusal> {
    if !has(AUTHOR_PERMISSION) {
        return Some(Refusal::permission(AUTHOR_PERMISSION));
    }
    lifecycle
        .check(Action::Edit, by)
        .err()
        .map(|refused| Refusal::from(&refused))
}

/// One file of a package version, as the Studio lists it to open in an editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFile {
    /// Its path in the package.
    pub path: String,
    /// Its `resourceType`, for a FHIR artefact.
    pub resource_type: Option<String>,
    /// Its canonical URL, if it has one.
    pub url: Option<String>,
    /// Its version, if it has one.
    pub version: Option<String>,
}

/// Every file of `packed` but the generated index, by path.
fn files(packed: &Packed) -> Vec<PackageFile> {
    packed
        .files
        .keys()
        .filter(|path| path.as_str() != INDEX)
        .map(|path| {
            let artefact = packed.contents.artefacts.iter().find(|a| &a.path == path);
            PackageFile {
                path: path.clone(),
                resource_type: artefact.map(|a| a.resource_type.clone()),
                url: artefact.and_then(|a| a.url.clone()),
                version: artefact.and_then(|a| a.version.clone()),
            }
        })
        .collect()
}

/// A version active in a scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveIn {
    /// Where.
    pub scope: Scope,
    /// Which version.
    pub version: Version,
}

/// One row of the package list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSummary {
    /// `name@version`.
    pub id: String,
    /// The package name.
    pub name: String,
    /// Its title, from its manifest Library.
    pub title: Option<String>,
    /// Its kind.
    pub kind: PackageKind,
    /// This version.
    pub version: Version,
    /// The latest published version before it, if any.
    pub previous: Option<Version>,
    /// How far the version moved from `previous`; `None` for a first version.
    pub bump: Option<Bump>,
    /// Where it stands.
    pub stage: Stage,
    /// Its Draft Holder.
    pub holder: String,
    /// Who it is being handed over to, until they accept or the offer is called off.
    pub offered_to: Option<String>,
    /// Everyone who has held it.
    pub authors: Vec<String>,
    /// The scopes the package is active in now, with the version active in each.
    pub active: Vec<ActiveIn>,
    /// Every action, each with why the signed-in person may not take it.
    pub actions: Vec<Allowed>,
    /// The first action they may take.
    pub next: Option<StudioAction>,
}

/// How an artefact changed against the previous version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtefactChangeKind {
    /// It is new.
    Added,
    /// Its content changed.
    Changed,
    /// It is gone.
    Removed,
}

/// One added, changed or removed artefact. Unchanged ones are left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtefactChange {
    /// How it changed.
    pub change: ArtefactChangeKind,
    /// Its `resourceType`.
    pub resource_type: String,
    /// Its path in the package.
    pub path: String,
    /// Its canonical URL, if it has one.
    pub url: Option<String>,
    /// Its version in the previous release.
    pub before: Option<String>,
    /// Its version in this one.
    pub after: Option<String>,
    /// For a changed artefact, each element that changed, in the order found; empty for an
    /// added or removed one.
    pub elements: Vec<ElementChange>,
}

/// One changed element of a changed artefact: the semver diff of metadata.md § Versioning,
/// summed up in a plain line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementChange {
    /// Its path, e.g. `item[bp].text` or `element[Patient.name].min`.
    pub at: String,
    /// How it changed.
    pub nature: Nature,
    /// The bump it needs.
    pub bump: Bump,
    /// One plain line, e.g. "Item bp: text reworded".
    pub summary: String,
}

/// Which automated check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckKind {
    /// The publication rules, the schemas and the semver check.
    Validation,
    /// The package's `tests/` in the sandbox.
    Simulation,
}

/// The latest stored result of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    /// Which check.
    pub check: CheckKind,
    /// Whether it passed.
    pub passed: bool,
    /// Every problem it found.
    pub problems: Vec<String>,
    /// When it ran.
    pub at: Timestamp,
    /// Whether it ran on the current content; false once the draft was revised.
    pub current: bool,
}

/// A package version read from top to bottom: what changed, is it safe, who must act, and the
/// discussion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackagePage {
    /// Its row in the list.
    pub summary: PackageSummary,
    /// What changed against [`PackageSummary::previous`].
    pub changes: Vec<ArtefactChange>,
    /// The stored check results, validation first.
    pub checks: Vec<CheckResult>,
    /// How many approvals its kind needs.
    pub approvals_required: usize,
    /// Who accepted it at review.
    pub reviewer: Option<String>,
    /// Who has approved it.
    pub approvers: Vec<String>,
    /// Its history, oldest first.
    pub history: Vec<Record>,
    /// Its revision: a save names it, and is refused once another came first.
    pub revision: u64,
    /// Why the signed-in person may not change it; `None` if they may.
    pub edit: Option<Refusal>,
    /// Its files, to open in an editor.
    pub files: Vec<PackageFile>,
    /// Its holder's change note for the whole draft.
    pub note: Option<String>,
}

/// The latest stored result of each check in `history`.
fn checks(lifecycle: &Lifecycle, history: &[Record]) -> Vec<CheckResult> {
    let mut found: BTreeMap<u8, CheckResult> = BTreeMap::new();
    for record in history {
        let (order, check, problems) = match &record.step {
            Step::Validated => (0, CheckKind::Validation, Vec::new()),
            Step::ValidationFailed { problems } => (0, CheckKind::Validation, problems.clone()),
            Step::Simulated => (1, CheckKind::Simulation, Vec::new()),
            Step::SimulationFailed { problems } => (1, CheckKind::Simulation, problems.clone()),
            Step::Checked {
                validation,
                simulation,
            } => {
                let result = |check, problems: &Vec<String>| CheckResult {
                    check,
                    passed: problems.is_empty(),
                    problems: problems.clone(),
                    at: record.at,
                    current: record.content == lifecycle.content,
                };
                found.insert(0, result(CheckKind::Validation, validation));
                found.insert(1, result(CheckKind::Simulation, simulation));
                continue;
            }
            _ => continue,
        };
        found.insert(
            order,
            CheckResult {
                check,
                passed: problems.is_empty(),
                problems,
                at: record.at,
                current: record.content == lifecycle.content,
            },
        );
    }
    found.into_values().collect()
}

/// The artefacts of `packed` by canonical URL, or by path when it has none; the manifest is
/// left out, since it follows the package's own version.
fn artefacts(packed: &Packed) -> BTreeMap<String, (ArtefactChange, String)> {
    let prints = fingerprints(&packed.files);
    packed
        .contents
        .artefacts
        .iter()
        .filter(|artefact| artefact.path != MANIFEST)
        .map(|artefact| {
            let key = artefact
                .url
                .clone()
                .unwrap_or_else(|| artefact.path.clone());
            let change = ArtefactChange {
                change: ArtefactChangeKind::Changed,
                resource_type: artefact.resource_type.clone(),
                path: artefact.path.clone(),
                url: artefact.url.clone(),
                before: None,
                after: artefact.version.clone(),
                elements: Vec::new(),
            };
            (
                key,
                (
                    change,
                    prints.get(&artefact.path).cloned().unwrap_or_default(),
                ),
            )
        })
        .collect()
}

/// Each element that changed from the file at `old` in `before` to the one at `new` in `after`.
/// A file that is not JSON changed as a whole.
fn elements(before: &Packed, old: &str, after: &Packed, new: &str) -> Vec<ElementChange> {
    let json = |packed: &Packed, path: &str| {
        packed
            .files
            .get(path)
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
    };
    let found = match (json(before, old), json(after, new)) {
        (Some(old), Some(new)) => health_packages::bump::changes(&old, &new),
        _ => vec![health_configuration::lifecycle::semver::ContentChange {
            at: String::new(),
            nature: Nature::Changed,
            bump: Bump::Major,
        }],
    };
    found
        .into_iter()
        .map(|change| ElementChange {
            summary: health_packages::bump::describe(&change),
            at: change.at,
            nature: change.nature,
            bump: change.bump,
        })
        .collect()
}

/// Each artefact added, changed or removed from `before` to `after`, a changed one with the
/// elements that changed in it.
pub fn changes(before: Option<&Packed>, after: &Packed) -> Vec<ArtefactChange> {
    let old = before.map(artefacts).unwrap_or_default();
    let new = artefacts(after);
    let mut found = Vec::new();
    for (key, (mut change, print)) in new.clone() {
        match old.get(&key) {
            None => {
                change.change = ArtefactChangeKind::Added;
                found.push(change);
            }
            Some((was, old_print)) if *old_print != print => {
                change.before.clone_from(&was.after);
                if let Some(before) = before {
                    change.elements = elements(before, &was.path, after, &change.path);
                }
                found.push(change);
            }
            Some(_) => {}
        }
    }
    for (key, (mut change, _)) in old {
        if !new.contains_key(&key) {
            change.change = ArtefactChangeKind::Removed;
            change.before = change.after.take();
            found.push(change);
        }
    }
    found
}

/// The title in `packed`'s manifest Library, if any.
fn title(packed: &Packed) -> Option<String> {
    let manifest: serde_json::Value = serde_json::from_slice(packed.files.get(MANIFEST)?).ok()?;
    manifest["title"].as_str().map(str::to_owned)
}

/// The scopes `package` is active in at `now`: in each scope, the latest issued activation
/// already in effect.
fn active(activations: &[ActivationRecord], package: &str, now: Timestamp) -> Vec<ActiveIn> {
    let mut by_scope: BTreeMap<Scope, (Timestamp, Version)> = BTreeMap::new();
    for record in activations {
        let activation = &record.activation;
        if activation.package != package || activation.effective_at > now {
            continue;
        }
        let later = by_scope
            .get(&activation.scope)
            .is_none_or(|(issued, _)| activation.issued_at >= *issued);
        if later {
            by_scope.insert(
                activation.scope.clone(),
                (activation.issued_at, activation.version.clone()),
            );
        }
    }
    by_scope
        .into_iter()
        .map(|(scope, (_, version))| ActiveIn { scope, version })
        .collect()
}

impl Publications {
    async fn activation_records(&self, tx: &mut SurrealTx) -> Result<Vec<ActivationRecord>, Error> {
        tx.package_activations()
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect()
    }

    /// The latest published version of `lifecycle`'s package before its own.
    pub(crate) async fn previous(
        &self,
        tx: &mut SurrealTx,
        lifecycle: &Lifecycle,
    ) -> Result<Option<PublishedPackage>, Error> {
        Ok(self
            .published(tx, &lifecycle.package)
            .await?
            .into_iter()
            .filter(|published| published.version < lifecycle.version)
            .max_by(|a, b| a.version.cmp(&b.version)))
    }

    async fn summary(
        &self,
        tx: &mut SurrealTx,
        lifecycle: &Lifecycle,
        packed: &Packed,
        activations: &[ActivationRecord],
        by: &str,
        has: &(dyn Fn(&str) -> bool + Sync),
    ) -> Result<(PackageSummary, Option<PublishedPackage>), Error> {
        let previous = self.previous(tx, lifecycle).await?;
        let actions = allowed(lifecycle, by, has);
        let summary = PackageSummary {
            id: lifecycle_id(lifecycle),
            name: lifecycle.package.clone(),
            title: title(packed),
            kind: lifecycle.kind,
            version: lifecycle.version.clone(),
            bump: previous
                .as_ref()
                .map(|p| Bump::between(&p.version, &lifecycle.version)),
            previous: previous.as_ref().map(|p| p.version.clone()),
            stage: lifecycle.stage,
            holder: lifecycle.holder.clone(),
            offered_to: lifecycle.offered_to.clone(),
            authors: lifecycle.authors.iter().cloned().collect(),
            active: active(activations, &lifecycle.package, self.clock.now()),
            next: next_step(&actions),
            actions,
        };
        Ok((summary, previous))
    }

    /// The package list for `by`, whose roles carry the permissions `has` answers for: one row
    /// per package, its newest version not withdrawn (or its newest withdrawn one, when it has
    /// nothing else), by name.
    pub async fn studio_packages(
        &self,
        by: &str,
        has: &(dyn Fn(&str) -> bool + Sync),
    ) -> Result<Vec<PackageSummary>, Error> {
        let mut tx = self.begin().await?;
        let mut newest: BTreeMap<String, Lifecycle> = BTreeMap::new();
        for json in tx.package_lifecycles().await.map_err(failed)? {
            let lifecycle: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
            let withdrawn = |l: &Lifecycle| l.stage == Stage::Withdrawn;
            let newer = newest.get(&lifecycle.package).is_none_or(|held| {
                (withdrawn(held), &lifecycle.version, lifecycle.withdrawn_at)
                    > (withdrawn(&lifecycle), &held.version, held.withdrawn_at)
            });
            if newer {
                newest.insert(lifecycle.package.clone(), lifecycle);
            }
        }
        let activations = self.activation_records(&mut tx).await?;
        let mut rows = Vec::with_capacity(newest.len());
        for lifecycle in newest.values() {
            let packed = self.packed(&lifecycle.content).await?;
            let (summary, _) = self
                .summary(&mut tx, lifecycle, &packed, &activations, by, has)
                .await?;
            rows.push(summary);
        }
        Ok(rows)
    }

    /// Package version `id` (`name@version`) read from top to bottom, for `by`.
    pub async fn studio_package(
        &self,
        id: &str,
        by: &str,
        has: &(dyn Fn(&str) -> bool + Sync),
    ) -> Result<PackagePage, Error> {
        let (lifecycle, history) = self.lifecycle(id).await?;
        let mut tx = self.begin().await?;
        let (_, revision) = self.load(&mut tx, id).await?;
        let activations = self.activation_records(&mut tx).await?;
        let packed = self.packed(&lifecycle.content).await?;
        let (summary, previous) = self
            .summary(&mut tx, &lifecycle, &packed, &activations, by, has)
            .await?;
        let before = match &previous {
            Some(published) => Some(self.packed(&published.digest).await?),
            None => None,
        };
        Ok(PackagePage {
            summary,
            changes: changes(before.as_ref(), &packed),
            checks: checks(&lifecycle, &history),
            approvals_required: lifecycle.approvals_required(),
            reviewer: lifecycle.reviewer.clone(),
            approvers: lifecycle.approvers.iter().cloned().collect(),
            history,
            revision,
            edit: edit_refusal(&lifecycle, by, has),
            files: files(&packed),
            note: lifecycle.note.clone(),
        })
    }
}
