//! Central's publication lifecycle (configuration.md § Lifecycle; ADR 0025; #88): Published
//! Definition packages move from Draft through Validation, Simulation, Review and Approval to a
//! signed publication. The rules are `health_configuration::lifecycle`; this crate keeps the
//! lifecycles in SurrealDB (migration `0027_package_lifecycle`) and their tarballs in the blob
//! store, and audits every transition.
//!
//! - [`Publications::draft`] packs an author's files (`health_packages::pack`) and stores the
//!   unsigned tarball as a blob. Its sha-256 is the draft's content: what is validated, reviewed,
//!   approved and finally signed. A second draft of the same `name@version` revises it, by its
//!   Draft Holder only.
//! - **The Draft Holder** (#493): [`Publications::hand_over`] offers a draft to someone,
//!   [`Publications::accept_handover`] makes them its holder and [`Publications::decline_handover`]
//!   calls the offer off; [`Publications::reassign`] is a manager's, with a reason that the
//!   history and the audit keep. [`Publications::withdraw`] ends a draft and keeps it under
//!   another id ([`lifecycle_id`]), so its `name@version` is free for a new draft.
//! - [`Publications::submit`] submits the draft and runs the automated steps in the same
//!   transaction:
//!   - **validation:** the publication rules and the release's schemas
//!     (`health_packages::check`), then the semver check against the latest published version
//!     of the package: the package's bump, and each changed artefact's own bump against its
//!     content (`health_packages::bump`);
//!   - **simulation:** the [`Sandbox`] runs the package's `tests/` cases: [`WorkflowSandbox`]
//!     runs its workflow scenarios (#101). [`NoSandbox`] runs none.
//!
//!   Either failing returns the draft to Draft, with every problem in the record.
//! - [`Publications::review`], [`Publications::approve`] and [`Publications::request_changes`]
//!   are the people's steps, with the separation of duties the domain enforces.
//! - [`Publications::publish`] signs the approved tarball with the publication-signing key
//!   ([`PublicationSigner`]), checking its sha-256 is still the approved one, and records the
//!   [`PublishedPackage`], once: a published `name@version` never changes.
//!
//! - [`Publications::activate`] issues a scoped, dated activation of a published version
//!   (ADR 0025; #89), with the tarball's sha-256 and signature so nodes can fetch and verify it.
//!   It is kept once and logged for every node below, and audited. Rollback is another
//!   activation, of an earlier version.
//!
//! - [`Publications::translation_report`] checks a Language Pack against the latest published
//!   version of its target (ADR 0066; #502): which entries are current, stale or gone.
//!   Validation refuses a pack whose target has no published version in the major it targets,
//!   and the semver check compares its translations (`health_configuration::language`).
//!
//! - **Override points** (ADR 0026, ADR 0067; #507): validation checks an override package's
//!   settings against the latest published version of each base package they name, and
//!   [`Publications::activate`] refuses a base version that would orphan, or put outside its
//!   values, a setting of an override in effect in an overlapping scope.
//!
//! - [`studio`] is what the Studio shows of it (#492): the package list, one package read from
//!   top to bottom, and the actions the signed-in person may take, each refused one with why.
//! - [`editing`] is how the Studio changes a draft (#494): one file at a time against the
//!   draft's revision, a new draft blank or copied from a published version, a change of
//!   version, and *Check now*.
//!
//! Each change is a compare-and-set on the lifecycle's revision, so of two people acting at
//! once one gets [`Error::Conflict`] rather than overwriting the other.
//!
//! The `central-server package` commands build theirs [`Publications::from_cli`], so the audit
//! of each step they take records it as a CLI action by the `--by` name.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use health_configuration::activation::{Activation, ActivationRecord, Scope, in_effect};
use health_configuration::language::{Report, Translates, Translations};
use health_configuration::lifecycle::semver::{self, BumpProblem, ContentChange, Release};
use health_configuration::lifecycle::{Lifecycle, Record, Refused, Stage, Step};
use health_configuration::overrides;
use health_configuration::package::{PackageKind, Version};
use health_files::{BlobHash, FsBlobStore};
use health_packages::{Limits, Packed, PublicationSigner, Schemas};
use health_persistence::{Store, Transaction};
use health_surrealdb::{SurrealStore, SurrealTx};
use health_time::{Clock, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest as _;
use uuid::Uuid;

pub mod decisions;
pub mod editing;
mod extraction;
mod programs;
pub mod studio;
pub mod terminology;
mod workflows;

pub use extraction::FIXTURE_TYPE as EXTRACTION_FIXTURE_TYPE;
pub use workflows::{WorkflowSandbox, workflow_definitions};

pub use health_workflow_actions::ActionDescriptor;

pub use health_packages::{Files, LocalSigner};

/// Why a publication step failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No lifecycle or package by that id.
    #[error("{0}")]
    NotFound(String),
    /// The lifecycle refused the step.
    #[error(transparent)]
    Refused(#[from] Refused),
    /// The files are not a package, or publication refused them.
    #[error(transparent)]
    Package(#[from] health_packages::Error),
    /// That `name@version` is already published, or the files are another package's.
    #[error("{0}")]
    Conflict(String),
    /// A save named a revision of the draft that is no longer its revision: someone else saved
    /// first (studio.md § The authoring API, Editing).
    #[error("the draft changed meanwhile (it is at revision {current}); read it again")]
    Stale {
        /// The draft's revision now.
        current: u64,
    },
    /// What was sent cannot be saved in the draft: a path outside the package, or a change only
    /// another request makes.
    #[error("{0}")]
    Invalid(String),
    /// The store or the blob store failed.
    #[error("store: {0}")]
    Store(String),
}

fn failed(error: impl std::fmt::Display) -> Error {
    Error::Store(error.to_string())
}

/// What a sandbox's run answers: every failing case, as a problem.
pub type Simulation<'a> = Pin<Box<dyn Future<Output = Vec<String>> + Send + 'a>>;

/// What runs a package's `tests/` cases in a sandbox (configuration.md § Lifecycle, Simulation).
pub trait Sandbox: Send + Sync {
    /// Every failing case, as a problem; none if all pass.
    fn simulate<'a>(&'a self, package: &'a Packed) -> Simulation<'a>;
}

/// Runs no case, so simulation passes every package: for tests of the other steps.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSandbox;

impl Sandbox for NoSandbox {
    fn simulate<'a>(&'a self, _: &'a Packed) -> Simulation<'a> {
        Box::pin(async { Vec::new() })
    }
}

/// A published package: what central distributes, and how it came to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedPackage {
    /// `name@version`.
    pub id: String,
    /// The package name.
    pub package: String,
    /// Its version.
    pub version: Version,
    /// Its kind.
    pub kind: PackageKind,
    /// The tarball's sha-256, hex: also its blob.
    pub digest: String,
    /// The detached signature (`<name>-<version>.tgz.sig`).
    pub signature: String,
    /// Who published it.
    pub published_by: String,
    /// When.
    pub published_at: Timestamp,
    /// Its authors.
    pub authors: Vec<String>,
    /// Who reviewed it.
    pub reviewer: Option<String>,
    /// Who approved it.
    pub approvers: Vec<String>,
}

/// Central's publication lifecycle over SurrealDB and the blob store.
#[derive(Clone)]
pub struct Publications {
    store: SurrealStore,
    blobs: FsBlobStore,
    clock: Arc<dyn Clock>,
    schemas: Arc<Schemas>,
    sandbox: Arc<dyn Sandbox>,
    limits: Limits,
    observer: String,
    actions: Arc<Vec<ActionDescriptor>>,
    cli: bool,
    read: terminology::Read,
}

/// What the CLI's steps are audited as coming through, beside the `--by` name.
pub const CLI_AGENT: &str = "central-server package (CLI)";

/// The id lifecycle `lifecycle` is kept under: `name@version`, or, once withdrawn,
/// `name@version~withdrawn-<when>`, which leaves `name@version` free for a new draft.
pub fn lifecycle_id(lifecycle: &Lifecycle) -> String {
    let id = format!("{}@{}", lifecycle.package, lifecycle.version);
    match (lifecycle.stage, lifecycle.withdrawn_at) {
        (Stage::Withdrawn, Some(at)) => {
            let when: String = at
                .to_string()
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            format!("{id}~withdrawn-{when}")
        }
        _ => id,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The sha-256 of each file, hex, by path: what the semver check compares.
fn fingerprints(files: &Files) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|(path, bytes)| (path.clone(), hex(&sha2::Sha256::digest(bytes))))
        .collect()
}

/// What the semver check compares of `packed`: a Language Pack's translations in place of its
/// entries file. `None` for an inexact version, already a publication problem.
fn release(packed: &Packed) -> Option<Release> {
    let release = Release::of(&packed.contents, &fingerprints(&packed.files)).ok()?;
    let package = &packed.contents.package;
    let major = package.translates.as_ref().and_then(Translates::major);
    Some(match (package.kind, major) {
        (PackageKind::Language, Some(major)) => {
            let entries = health_packages::language::translations(&packed.files)
                .ok()
                .flatten()
                .map(|read| read.entries)
                .unwrap_or_default();
            release.with_translations(Translations::of(major, &entries))
        }
        _ => release,
    })
}

/// A Language Pack checked against its target (ADR 0066).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TranslationReport {
    /// The target version checked against, `name@version`.
    pub target: String,
    /// Which entries are current, stale or gone.
    pub report: Report,
}

/// One problem per line, with an understated artefact's changes under it.
fn describe(problem: &BumpProblem) -> String {
    match problem {
        BumpProblem::ArtefactUnderstated { changes, .. } => {
            let detail: Vec<String> = changes
                .iter()
                .map(|change| format!("{} ({}, {})", change.at, nature(change), change.bump))
                .collect();
            format!("{problem}: {}", detail.join(", "))
        }
        other => other.to_string(),
    }
}

fn nature(change: &ContentChange) -> String {
    serde_json::to_value(change.nature)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Publications {
    /// The lifecycle over `store` and `blobs`, checking configuration against `schemas`,
    /// simulating in `sandbox`, audited as `observer`.
    pub fn new(
        store: SurrealStore,
        blobs: FsBlobStore,
        clock: Arc<dyn Clock>,
        schemas: Schemas,
        sandbox: Arc<dyn Sandbox>,
        observer: impl Into<String>,
    ) -> Self {
        Self {
            store,
            blobs,
            clock,
            schemas: Arc::new(schemas),
            sandbox,
            limits: Limits::default(),
            observer: observer.into(),
            actions: Arc::new(Vec::new()),
            cli: false,
            read: terminology::Read::default(),
        }
    }

    /// These publications, as the `central-server package` commands use them: each step is
    /// audited as a CLI action, by the `--by` name it was given.
    #[must_use]
    pub fn from_cli(mut self) -> Self {
        self.cli = true;
        self
    }

    /// These publications, checking Workflow Definitions against `actions`: every workflow
    /// action the release knows (#96). Without it, a definition that executes any action fails
    /// validation.
    #[must_use]
    pub fn with_actions(mut self, actions: Vec<ActionDescriptor>) -> Self {
        self.actions = Arc::new(actions);
        self
    }

    /// The workflow actions definitions are checked against: what the Studio offers (#498).
    pub fn actions(&self) -> &[ActionDescriptor] {
        &self.actions
    }

    /// These publications, running `tests/` cases in `sandbox` instead.
    #[must_use]
    pub fn with_sandbox(mut self, sandbox: Arc<dyn Sandbox>) -> Self {
        self.sandbox = sandbox;
        self
    }

    fn audit(&self, record: &Record, detail: &str) -> Value {
        let mut agents = vec![json!({ "who": { "display": record.by }, "requestor": true })];
        if self.cli {
            agents.push(json!({ "who": { "display": CLI_AGENT }, "requestor": false }));
        }
        json!({
            "resourceType": "AuditEvent",
            "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/audit-event-type", "code": "rest", "display": "Restful Operation" }] }],
            "code": { "coding": [{ "system": "urn:health:audit-event", "code": "package-lifecycle" }] },
            "action": if record.step == Step::Created { "C" } else { "U" },
            "severity": "notice",
            "recorded": record.at.to_string(),
            "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
            "agent": agents,
            "source": { "observer": { "display": self.observer } },
            "entity": [{
                "what": { "display": format!("package {}@{}", record.package, record.version) },
                "detail": [
                    { "type": { "text": "transition" }, "valueString": format!("{} → {}", record.from, record.to) },
                    { "type": { "text": "content" }, "valueString": record.content },
                    { "type": { "text": "change" }, "valueString": detail },
                ],
            }],
        })
    }

    async fn begin(&self) -> Result<SurrealTx, Error> {
        self.store.begin().await.map_err(failed)
    }

    async fn put_blob(&self, bytes: Vec<u8>) -> Result<(), Error> {
        let blobs = self.blobs.clone();
        tokio::task::spawn_blocking(move || blobs.put(bytes.as_slice()).map(|_| ()))
            .await
            .map_err(failed)?
            .map_err(failed)
    }

    async fn read_blob(&self, digest: &str) -> Result<Vec<u8>, Error> {
        let hash: BlobHash = digest
            .parse()
            .map_err(|_| Error::Store(format!("{digest} is not a sha-256")))?;
        let blobs = self.blobs.clone();
        tokio::task::spawn_blocking(move || blobs.read(&hash))
            .await
            .map_err(failed)?
            .map_err(failed)
    }

    async fn packed(&self, digest: &str) -> Result<Packed, Error> {
        let tgz = self.read_blob(digest).await?;
        Ok(health_packages::unpack(&tgz, self.limits)?)
    }

    async fn load(&self, tx: &mut SurrealTx, id: &str) -> Result<(Lifecycle, u64), Error> {
        let (json, revision) = tx
            .package_lifecycle(id)
            .await
            .map_err(failed)?
            .ok_or_else(|| Error::NotFound(format!("no package {id} in the lifecycle")))?;
        Ok((serde_json::from_str(&json).map_err(failed)?, revision))
    }

    /// Writes `lifecycle` over `before` (`None` for a new one), and each record with its audit.
    async fn save(
        &self,
        mut tx: SurrealTx,
        lifecycle: &Lifecycle,
        before: Option<u64>,
        records: &[(Record, String)],
    ) -> Result<(), Error> {
        let id = format!("{}@{}", lifecycle.package, lifecycle.version);
        let data = serde_json::to_string(lifecycle).map_err(failed)?;
        let stage = lifecycle.stage.to_string();
        match before {
            None => tx
                .create_package_lifecycle(&id, &lifecycle.package, &stage, &data)
                .await
                .map_err(|_| Error::Conflict(format!("{id} is already in the lifecycle")))?,
            Some(revision) => {
                let updated = tx
                    .update_package_lifecycle(&id, &stage, &data, revision)
                    .await
                    .map_err(failed)?;
                if !updated {
                    return Err(Error::Conflict(format!(
                        "{id} changed meanwhile; read it again"
                    )));
                }
            }
        }
        self.append(&mut tx, &id, records).await?;
        tx.commit().await.map_err(failed)
    }

    /// Appends each record to lifecycle `id`, with its audit.
    async fn append(
        &self,
        tx: &mut SurrealTx,
        id: &str,
        records: &[(Record, String)],
    ) -> Result<(), Error> {
        for (record, detail) in records {
            tx.append_package_lifecycle_record(
                &Uuid::now_v7().to_string(),
                id,
                &serde_json::to_string(record).map_err(failed)?,
            )
            .await
            .map_err(failed)?;
            health_audit::append(tx, &self.audit(record, detail))
                .await
                .map_err(failed)?;
        }
        Ok(())
    }

    /// Starts or revises the draft of the package in `files`, by `by`.
    pub async fn draft(&self, files: &Files, by: &str) -> Result<Lifecycle, Error> {
        let packed = health_packages::pack(files)?;
        let version: Version = packed
            .contents
            .package
            .version
            .parse()
            .map_err(|e| Error::Conflict(format!("the package version: {e}")))?;
        let kind = packed.contents.package.kind;
        let content = packed.digest.to_string();
        let now = self.clock.now();
        let mut tx = self.begin().await?;
        if tx
            .published_package(&packed.id)
            .await
            .map_err(failed)?
            .is_some()
        {
            return Err(Error::Conflict(format!(
                "{} is published, and never changes: draft a new version",
                packed.id
            )));
        }
        let existing = tx.package_lifecycle(&packed.id).await.map_err(failed)?;
        self.put_blob(packed.tgz.clone()).await?;
        match existing {
            None => {
                let (lifecycle, record) = Lifecycle::create(
                    &packed.contents.package.name,
                    version,
                    kind,
                    by,
                    &content,
                    now,
                )?;
                let records = [(record, format!("drafted, {content}"))];
                self.save(tx, &lifecycle, None, &records).await?;
                Ok(lifecycle)
            }
            Some((json, revision)) => {
                let mut lifecycle: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
                if lifecycle.stage == Stage::Draft {
                    // In Draft the kind may change with the content; approvals start over anyway.
                    lifecycle.kind = kind;
                }
                let record = lifecycle.revise(by, &content, now)?;
                let records = [(record, format!("revised, {content}"))];
                self.save(tx, &lifecycle, Some(revision), &records).await?;
                Ok(lifecycle)
            }
        }
    }

    /// Every reason `packed` fails validation.
    async fn validation(&self, tx: &mut SurrealTx, packed: &Packed) -> Result<Vec<String>, Error> {
        let mut problems = Vec::new();
        match health_packages::check(packed, &self.schemas) {
            Ok(()) => {}
            Err(health_packages::Error::NotPublishable(found)) => {
                problems.extend(found.iter().map(ToString::to_string));
            }
            Err(other) => problems.push(other.to_string()),
        }
        problems.extend(workflows::problems(packed, &self.actions));
        problems.extend(extraction::problems(packed).await);
        problems.extend(programs::problems(packed));
        if packed.contents.package.kind == PackageKind::Language
            && let Some(translates) = &packed.contents.package.translates
            && translates.major().is_some()
            && self.target(tx, translates).await?.is_none()
        {
            problems.push(format!(
                "no version of {}@{} is published for the pack to translate",
                translates.package, translates.version
            ));
        }
        problems.extend(self.override_problems(tx, packed).await?);
        let name = &packed.contents.package.name;
        let latest = self
            .published(tx, name)
            .await?
            .into_iter()
            .max_by(|a, b| a.version.cmp(&b.version));
        let Some(latest) = latest else {
            return Ok(problems);
        };
        let before = self.packed(&latest.digest).await?;
        let (Some(old), Some(new)) = (release(&before), release(packed)) else {
            // An inexact version is already a publication problem.
            return Ok(problems);
        };
        let path_of = |packed: &Packed, url: &str| {
            packed
                .contents
                .artefacts
                .iter()
                .find(|artefact| artefact.url.as_deref() == Some(url))
                .map(|artefact| artefact.path.clone())
        };
        let document = |packed: &Packed, path: Option<String>| -> Option<Value> {
            serde_json::from_slice(packed.files.get(&path?)?).ok()
        };
        let mut content = BTreeMap::new();
        for (url, after) in &new.artefacts {
            let Some(was) = old.artefacts.get(url) else {
                continue;
            };
            if was.version == after.version {
                continue;
            }
            if let (Some(a), Some(b)) = (
                document(&before, path_of(&before, url)),
                document(packed, path_of(packed, url)),
            ) {
                content.insert(url.clone(), health_packages::bump::changes(&a, &b));
            }
        }
        if let Err(found) = semver::check(&old, &new, &content) {
            problems.extend(found.iter().map(describe));
        }
        Ok(problems)
    }

    /// An override package's settings checked against the latest published version of each
    /// base package they name (ADR 0026, ADR 0067): a base that is not published, a key it does
    /// not declare and a value it does not allow are refused.
    async fn override_problems(
        &self,
        tx: &mut SurrealTx,
        packed: &Packed,
    ) -> Result<Vec<String>, Error> {
        if packed.contents.package.kind != PackageKind::Override {
            return Ok(Vec::new());
        }
        let Ok(settings) = health_packages::overrides::settings(&packed.files) else {
            // An unreadable file is already a publication problem.
            return Ok(Vec::new());
        };
        let mut problems = Vec::new();
        for base in overrides::bases(&settings) {
            let latest = self
                .published(tx, base)
                .await?
                .into_iter()
                .max_by(|a, b| a.version.cmp(&b.version));
            let Some(latest) = latest else {
                problems.push(format!("no version of {base} is published to override"));
                continue;
            };
            let declared = self.packed(&latest.digest).await?;
            match health_packages::overrides::points(&declared.files) {
                Ok(points) => problems.extend(
                    overrides::check(base, &points, &settings)
                        .iter()
                        .map(|problem| format!("{problem} (in {})", latest.id)),
                ),
                Err(error) => problems.push(error.to_string()),
            }
        }
        Ok(problems)
    }

    /// What stops published base package `published` being activated in `scope`: each override
    /// package in effect in a scope overlapping it whose settings for the base the version would
    /// orphan, or put outside the values it allows (ADR 0026, ADR 0067). The scopes do not say
    /// which district a facility is in, so a district's base and a facility's override (or the
    /// reverse) are checked by the node when it switches.
    async fn override_conflicts(
        &self,
        tx: &mut SurrealTx,
        published: &PublishedPackage,
        scope: &Scope,
    ) -> Result<Vec<String>, Error> {
        if matches!(
            published.kind,
            PackageKind::Override | PackageKind::Language
        ) {
            return Ok(Vec::new());
        }
        let records: Vec<ActivationRecord> = tx
            .package_activations()
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect::<Result<_, _>>()?;
        let activations: Vec<Activation> = records.into_iter().map(|r| r.activation).collect();
        let mut base = None;
        let mut problems = Vec::new();
        for active in in_effect(&activations, self.clock.now()) {
            if active.package == published.package || !active.scope.overlaps(scope) {
                continue;
            }
            let id = format!("{}@{}", active.package, active.version);
            let Some(json) = tx.published_package(&id).await.map_err(failed)? else {
                continue;
            };
            let found: PublishedPackage = serde_json::from_str(&json).map_err(failed)?;
            if found.kind != PackageKind::Override {
                continue;
            }
            let settings = self.packed(&found.digest).await?;
            if base.is_none() {
                base = Some(self.packed(&published.digest).await?);
            }
            let Some(base) = &base else { continue };
            problems.extend(
                health_packages::overrides::check(
                    &published.package,
                    &base.files,
                    &settings.files,
                )?
                .iter()
                .map(|problem| format!("{id} in {}: {problem}", active.scope)),
            );
        }
        Ok(problems)
    }

    async fn published(
        &self,
        tx: &mut SurrealTx,
        package: &str,
    ) -> Result<Vec<PublishedPackage>, Error> {
        tx.published_versions(package)
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect()
    }

    /// An author submits `id` (`name@version`): it is validated and simulated at once, and
    /// either reaches Review or returns to Draft with its problems in the records.
    pub async fn submit(&self, id: &str, by: &str) -> Result<Lifecycle, Error> {
        let mut tx = self.begin().await?;
        let (mut lifecycle, revision) = self.load(&mut tx, id).await?;
        let mut records = Vec::new();
        let record = lifecycle.submit(by, self.clock.now())?;
        records.push((record, "submitted".to_owned()));
        let packed = self.packed(&lifecycle.content).await?;
        let problems = self.validation(&mut tx, &packed).await?;
        let detail = if problems.is_empty() {
            "validated".to_owned()
        } else {
            format!("validation failed: {}", problems.join("; "))
        };
        let record = lifecycle.validated(problems, self.clock.now())?;
        records.push((record, detail));
        if lifecycle.stage == Stage::Simulation {
            let failures = self.sandbox.simulate(&packed).await;
            let detail = if failures.is_empty() {
                "simulated".to_owned()
            } else {
                format!("simulation failed: {}", failures.join("; "))
            };
            let record = lifecycle.simulated(failures, self.clock.now())?;
            records.push((record, detail));
        }
        self.save(tx, &lifecycle, Some(revision), &records).await?;
        Ok(lifecycle)
    }

    async fn step(
        &self,
        id: &str,
        act: impl FnOnce(&mut Lifecycle, Timestamp) -> Result<(Record, String), Refused>,
    ) -> Result<Lifecycle, Error> {
        let mut tx = self.begin().await?;
        let (mut lifecycle, revision) = self.load(&mut tx, id).await?;
        let record = act(&mut lifecycle, self.clock.now())?;
        self.save(tx, &lifecycle, Some(revision), &[record]).await?;
        Ok(lifecycle)
    }

    /// `by`, not an author, accepts `id` at review.
    pub async fn review(&self, id: &str, by: &str) -> Result<Lifecycle, Error> {
        self.step(id, |lifecycle, now| {
            Ok((lifecycle.review(by, now)?, "reviewed".to_owned()))
        })
        .await
    }

    /// `by`, not an author, sends `id` back to Draft for `reason`.
    pub async fn request_changes(
        &self,
        id: &str,
        by: &str,
        reason: &str,
    ) -> Result<Lifecycle, Error> {
        self.step(id, |lifecycle, now| {
            let record = lifecycle.request_changes(by, reason, now)?;
            Ok((record, format!("changes requested: {reason}")))
        })
        .await
    }

    /// `by`, not an author, approves `id`.
    pub async fn approve(&self, id: &str, by: &str) -> Result<Lifecycle, Error> {
        self.step(id, |lifecycle, now| {
            let record = lifecycle.approve(by, now)?;
            let detail = format!(
                "approved ({} of {})",
                lifecycle.approvers.len(),
                lifecycle.approvals_required()
            );
            Ok((record, detail))
        })
        .await
    }

    /// The holder, `by`, offers draft `id` to `to`, who holds it once they accept.
    pub async fn hand_over(&self, id: &str, by: &str, to: &str) -> Result<Lifecycle, Error> {
        self.step(id, |lifecycle, now| {
            let record = lifecycle.hand_over(by, to, now)?;
            Ok((record, format!("offered to {}", to.trim())))
        })
        .await
    }

    /// `by`, offered draft `id`, accepts it and holds it from now on.
    pub async fn accept_handover(&self, id: &str, by: &str) -> Result<Lifecycle, Error> {
        self.step(id, |lifecycle, now| {
            let from = lifecycle.holder.clone();
            let record = lifecycle.accept_handover(by, now)?;
            Ok((record, format!("handed over from {from}")))
        })
        .await
    }

    /// The holder of draft `id`, or the person offered it, calls the handover off.
    pub async fn decline_handover(&self, id: &str, by: &str) -> Result<Lifecycle, Error> {
        self.step(id, |lifecycle, now| {
            let record = lifecycle.decline_handover(by, now)?;
            Ok((record, "handover called off".to_owned()))
        })
        .await
    }

    /// A manager, `by`, gives draft `id` to `to`, its holder being unavailable, for `reason`.
    /// Whether `by` may manage drafts is the caller's to check (`metadata.manage`).
    pub async fn reassign(
        &self,
        id: &str,
        by: &str,
        to: &str,
        reason: &str,
    ) -> Result<Lifecycle, Error> {
        self.step(id, |lifecycle, now| {
            let from = lifecycle.holder.clone();
            let record = lifecycle.reassign(by, to, reason, now)?;
            let detail = format!("reassigned from {from} to {}: {}", to.trim(), reason.trim());
            Ok((record, detail))
        })
        .await
    }

    /// The holder, `by`, withdraws `id` before publication. It is kept, with its history, under
    /// the id [`lifecycle_id`] gives it, which this answers; `id` is free for a new draft.
    pub async fn withdraw(&self, id: &str, by: &str) -> Result<(String, Lifecycle), Error> {
        let mut tx = self.begin().await?;
        let (mut lifecycle, revision) = self.load(&mut tx, id).await?;
        let record = lifecycle.withdraw(by, self.clock.now())?;
        let kept = lifecycle_id(&lifecycle);
        let data = serde_json::to_string(&lifecycle).map_err(failed)?;
        let moved = tx
            .rekey_package_lifecycle(
                id,
                &kept,
                &lifecycle.package,
                &lifecycle.stage.to_string(),
                &data,
                revision,
            )
            .await
            .map_err(failed)?;
        if !moved {
            return Err(Error::Conflict(format!(
                "{id} changed meanwhile; read it again"
            )));
        }
        self.append(
            &mut tx,
            &kept,
            &[(record, format!("withdrawn, kept as {kept}"))],
        )
        .await?;
        tx.commit().await.map_err(failed)?;
        Ok((kept, lifecycle))
    }

    /// `by` publishes `id` once every approval is in, signing it with `signer`.
    pub async fn publish(
        &self,
        id: &str,
        by: &str,
        signer: &dyn PublicationSigner,
    ) -> Result<PublishedPackage, Error> {
        let mut tx = self.begin().await?;
        let (mut lifecycle, revision) = self.load(&mut tx, id).await?;
        lifecycle.ready()?;
        let packed = self.packed(&lifecycle.content).await?;
        let now = self.clock.now();
        let signed = health_packages::sign(&packed, &self.schemas, signer, now)?;
        let record = lifecycle.publish(by, &signed.digest.to_string(), now)?;
        let published = PublishedPackage {
            id: id.to_owned(),
            package: lifecycle.package.clone(),
            version: lifecycle.version.clone(),
            kind: lifecycle.kind,
            digest: signed.digest.to_string(),
            signature: signed.signature,
            published_by: by.to_owned(),
            published_at: now,
            authors: lifecycle.authors.iter().cloned().collect(),
            reviewer: lifecycle.reviewer.clone(),
            approvers: lifecycle.approvers.iter().cloned().collect(),
        };
        tx.create_published_package(
            id,
            &lifecycle.package,
            &serde_json::to_string(&published).map_err(failed)?,
        )
        .await
        .map_err(|_| Error::Conflict(format!("{id} is already published")))?;
        let detail = format!("published, signed {}", published.digest);
        self.save(tx, &lifecycle, Some(revision), &[(record, detail)])
            .await?;
        Ok(published)
    }

    /// Lifecycle `id` and its records, oldest first.
    pub async fn lifecycle(&self, id: &str) -> Result<(Lifecycle, Vec<Record>), Error> {
        let mut tx = self.begin().await?;
        let (lifecycle, _) = self.load(&mut tx, id).await?;
        let records = tx
            .package_lifecycle_records(id)
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect::<Result<_, _>>()?;
        Ok((lifecycle, records))
    }

    /// Every lifecycle not yet published nor withdrawn, by id.
    pub async fn open(&self) -> Result<Vec<Lifecycle>, Error> {
        let mut tx = self.begin().await?;
        tx.open_package_lifecycles()
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect()
    }

    /// `by` activates published `package@version` in `scope` from `effective_at` (now, if
    /// `None`), saying why. Activating a version that is not published is refused, as is a base
    /// version that conflicts with an override in effect there ([`Error::Conflict`]).
    pub async fn activate(
        &self,
        package: &str,
        version: &Version,
        scope: Scope,
        effective_at: Option<Timestamp>,
        by: &str,
        reason: Option<&str>,
    ) -> Result<ActivationRecord, Error> {
        let mut tx = self.begin().await?;
        let id = format!("{package}@{version}");
        let json = tx
            .published_package(&id)
            .await
            .map_err(failed)?
            .ok_or_else(|| Error::NotFound(format!("{id} is not published")))?;
        let published: PublishedPackage = serde_json::from_str(&json).map_err(failed)?;
        let conflicts = self.override_conflicts(&mut tx, &published, &scope).await?;
        if !conflicts.is_empty() {
            return Err(Error::Conflict(format!(
                "{id} cannot be activated in {scope} until its overrides there are removed or changed: {}",
                conflicts.join("; ")
            )));
        }
        let now = self.clock.now();
        let record = ActivationRecord {
            activation: Activation {
                id: Uuid::now_v7().to_string(),
                package: package.to_owned(),
                version: version.clone(),
                scope,
                effective_at: effective_at.unwrap_or(now),
                issued_at: now,
                by: by.to_owned(),
                reason: reason.map(str::to_owned),
            },
            digest: published.digest,
            signature: published.signature,
        };
        let activation = &record.activation;
        let mut agents = vec![json!({ "who": { "display": by }, "requestor": true })];
        if self.cli {
            agents.push(json!({ "who": { "display": CLI_AGENT }, "requestor": false }));
        }
        tx.put_package_activation(
            &activation.id,
            package,
            &now.to_string(),
            &serde_json::to_string(&record).map_err(failed)?,
        )
        .await
        .map_err(failed)?;
        let audit = json!({
            "resourceType": "AuditEvent",
            "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/audit-event-type", "code": "rest", "display": "Restful Operation" }] }],
            "code": { "coding": [{ "system": "urn:health:audit-event", "code": "package-activation" }] },
            "action": "C",
            "severity": "notice",
            "recorded": now.to_string(),
            "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
            "agent": agents,
            "source": { "observer": { "display": self.observer } },
            "entity": [{
                "what": { "display": format!("package {id}") },
                "detail": [
                    { "type": { "text": "scope" }, "valueString": activation.scope.to_string() },
                    { "type": { "text": "effective" }, "valueString": activation.effective_at.to_string() },
                    { "type": { "text": "reason" }, "valueString": reason.unwrap_or("") },
                ],
            }],
        });
        health_audit::append(&mut tx, &audit)
            .await
            .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        Ok(record)
    }

    /// Every activation issued, oldest first.
    pub async fn activations(&self) -> Result<Vec<ActivationRecord>, Error> {
        let mut tx = self.begin().await?;
        tx.package_activations()
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect()
    }

    /// The latest published version of the package `translates` names, in the major version it
    /// targets.
    async fn target(
        &self,
        tx: &mut SurrealTx,
        translates: &Translates,
    ) -> Result<Option<PublishedPackage>, Error> {
        let Some(major) = translates.major() else {
            return Ok(None);
        };
        Ok(self
            .published(tx, &translates.package)
            .await?
            .into_iter()
            .filter(|published| published.version.major == major)
            .max_by(|a, b| a.version.cmp(&b.version)))
    }

    /// Language Pack `id`, a draft or a published version, checked against the latest published
    /// version of its target in the major version it targets, and the published packages that
    /// version depends on (ADR 0066): which entries are current, which stale (their English
    /// changed, so English is served and the Studio lists them for re-translation), and which
    /// gone (their target no longer exists).
    pub async fn translation_report(&self, id: &str) -> Result<TranslationReport, Error> {
        let mut tx = self.begin().await?;
        let (lifecycle, _) = self.load(&mut tx, id).await?;
        let pack = self.packed(&lifecycle.content).await?;
        let translates = match (
            &pack.contents.package.kind,
            &pack.contents.package.translates,
        ) {
            (PackageKind::Language, Some(translates)) => translates.clone(),
            _ => return Err(Error::Conflict(format!("{id} is not a language pack"))),
        };
        let target = self.target(&mut tx, &translates).await?.ok_or_else(|| {
            Error::NotFound(format!(
                "no version of {}@{} is published",
                translates.package, translates.version
            ))
        })?;
        let mut packages = vec![self.packed(&target.digest).await?];
        let dependencies = packages[0].contents.package.dependencies.clone();
        for (name, version) in dependencies {
            if let Some(json) = tx
                .published_package(&format!("{name}@{version}"))
                .await
                .map_err(failed)?
            {
                let published: PublishedPackage = serde_json::from_str(&json).map_err(failed)?;
                packages.push(self.packed(&published.digest).await?);
            }
        }
        let files: Vec<&Files> = packages.iter().map(|packed| &packed.files).collect();
        Ok(TranslationReport {
            target: target.id,
            report: health_packages::language::report(&pack, &files)?,
        })
    }

    /// Published package `id`, with its tarball.
    pub async fn package(&self, id: &str) -> Result<(PublishedPackage, Vec<u8>), Error> {
        let mut tx = self.begin().await?;
        let json = tx
            .published_package(id)
            .await
            .map_err(failed)?
            .ok_or_else(|| Error::NotFound(format!("{id} is not published")))?;
        let published: PublishedPackage = serde_json::from_str(&json).map_err(failed)?;
        let tgz = self.read_blob(&published.digest).await?;
        Ok((published, tgz))
    }
}
