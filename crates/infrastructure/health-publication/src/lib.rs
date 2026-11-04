//! Central's publication lifecycle (configuration.md § Lifecycle; ADR 0025; #88): Published
//! Definition packages move from Draft through Validation, Simulation, Review and Approval to a
//! signed publication. The rules are `health_configuration::lifecycle`; this crate keeps the
//! lifecycles in SurrealDB (migration `0027_package_lifecycle`) and their tarballs in the blob
//! store, and audits every transition.
//!
//! - [`Publications::draft`] packs an author's files (`health_packages::pack`) and stores the
//!   unsigned tarball as a blob. Its sha-256 is the draft's content: what is validated, reviewed,
//!   approved and finally signed. A second draft of the same `name@version` revises it, and makes
//!   its author an author too.
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
//! Each change is a compare-and-set on the lifecycle's revision, so of two people acting at
//! once one gets [`Error::Conflict`] rather than overwriting the other.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_configuration::lifecycle::semver::{self, BumpProblem, ContentChange, Release};
use health_configuration::lifecycle::{Lifecycle, Record, Refused, Stage, Step};
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

mod workflows;

pub use workflows::WorkflowSandbox;

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
    /// The store or the blob store failed.
    #[error("store: {0}")]
    Store(String),
}

fn failed(error: impl std::fmt::Display) -> Error {
    Error::Store(error.to_string())
}

/// What runs a package's `tests/` cases in a sandbox (configuration.md § Lifecycle, Simulation).
pub trait Sandbox: Send + Sync {
    /// Every failing case, as a problem; none if all pass.
    fn simulate(&self, package: &Packed) -> Vec<String>;
}

/// Runs no case, so simulation passes every package: for tests of the other steps.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSandbox;

impl Sandbox for NoSandbox {
    fn simulate(&self, _: &Packed) -> Vec<String> {
        Vec::new()
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
        }
    }

    /// These publications, checking Workflow Definitions against `actions`: every workflow
    /// action the release knows (#96). Without it, a definition that executes any action fails
    /// validation.
    #[must_use]
    pub fn with_actions(mut self, actions: Vec<ActionDescriptor>) -> Self {
        self.actions = Arc::new(actions);
        self
    }

    fn audit(&self, record: &Record, detail: &str) -> Value {
        json!({
            "resourceType": "AuditEvent",
            "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/audit-event-type", "code": "rest", "display": "Restful Operation" }] }],
            "code": { "coding": [{ "system": "urn:health:audit-event", "code": "package-lifecycle" }] },
            "action": if record.step == Step::Created { "C" } else { "U" },
            "severity": "notice",
            "recorded": record.at.to_string(),
            "outcome": { "code": { "system": "http://terminology.hl7.org/CodeSystem/audit-event-outcome", "code": "0" } },
            "agent": [{ "who": { "display": record.by }, "requestor": true }],
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
        for (record, detail) in records {
            tx.append_package_lifecycle_record(
                &Uuid::now_v7().to_string(),
                &id,
                &serde_json::to_string(record).map_err(failed)?,
            )
            .await
            .map_err(failed)?;
            health_audit::append(&mut tx, &self.audit(record, detail))
                .await
                .map_err(failed)?;
        }
        tx.commit().await.map_err(failed)
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
        let (Ok(old), Ok(new)) = (
            Release::of(&before.contents, &fingerprints(&before.files)),
            Release::of(&packed.contents, &fingerprints(&packed.files)),
        ) else {
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
            let failures = self.sandbox.simulate(&packed);
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

    /// Every lifecycle not yet published, by id.
    pub async fn open(&self) -> Result<Vec<Lifecycle>, Error> {
        let mut tx = self.begin().await?;
        tx.open_package_lifecycles()
            .await
            .map_err(failed)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(failed))
            .collect()
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
