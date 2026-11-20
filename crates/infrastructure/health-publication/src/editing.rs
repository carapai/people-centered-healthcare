//! How the Studio changes a draft (studio.md § The authoring API, Editing, Starting, Checks;
//! #494).
//!
//! - **The working copy** is the draft's own tarball: [`Publications::draft_files`] reads its
//!   files, and [`Publications::save_file`] changes one of them, repacks the tarball and keeps
//!   it as the draft's new content.
//! - **The revision** counts the draft's changes (the lifecycle's compare-and-set). A save names
//!   the revision it read, sent by the client as `If-Match`; if someone saved since, it is
//!   refused with [`Error::Stale`] and nothing is kept, so a person reloads instead of
//!   overwriting someone else's change.
//! - **Starting:** [`Publications::start_copy`] copies a published version's files under the
//!   next minor version, marked `-draft` (`2.0.0` → `2.1.0-draft`); [`Publications::start_blank`]
//!   writes a new package's `package.json` and manifest. [`Publications::change_version`] gives
//!   a draft another version until it is submitted, where the semver check confirms it.
//! - **Check now:** [`Publications::check_now`] runs submission's validation and simulation and
//!   keeps the results as a record, without changing the stage or the revision.
//! - **The change note:** [`Publications::set_note`] keeps the holder's one note for the whole
//!   draft, shown in *What changed*, under the same revision check.
//!
//! Who may edit is the lifecycle's rule ([`Action::Edit`]); the permission is checked by the
//! API. Only the Draft Holder saves (#493).

use health_configuration::lifecycle::{Action, Lifecycle, Stage};
use health_configuration::package::{
    FHIR_VERSION, INDEX, MANIFEST, PACKAGE_JSON, PackageKind, Version, valid_name,
};
use health_packages::{FORMAT_VERSIONS, Files};
use health_persistence::Transaction as _;
use serde_json::{Value, json};

use crate::{Error, Publications, PublishedPackage, failed};

/// A new package's draft, started blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blank {
    /// Its name, e.g. `ug.moh.tb`.
    pub name: String,
    /// Its kind.
    pub kind: PackageKind,
    /// Its title, in its manifest.
    pub title: String,
    /// Its first version.
    pub version: Version,
    /// The canonical base its artefacts' URLs start with, e.g. `http://health.go.ug/fhir`.
    pub canonical: String,
    /// The software release it needs at least (`platform.minSoftware`).
    pub min_software: String,
}

/// The version a copy of `published` proposes: the next minor, marked as a draft.
pub fn next_version(published: &Version) -> Version {
    Version {
        major: published.major,
        minor: published.minor + 1,
        patch: 0,
        pre: Some("draft".into()),
    }
}

/// `bytes`, a JSON object, with its `version` set to `version`.
fn with_version(path: &str, bytes: &[u8], version: &Version) -> Result<Vec<u8>, Error> {
    let mut value: Value = serde_json::from_slice(bytes)
        .map_err(|e| Error::Invalid(format!("{path} does not read: {e}")))?;
    let Some(object) = value.as_object_mut() else {
        return Err(Error::Invalid(format!("{path} is not a JSON object")));
    };
    object.insert("version".into(), json!(version.to_string()));
    serde_json::to_vec_pretty(&value).map_err(failed)
}

/// `files` with `package.json` and the manifest at `version`.
fn renumbered(mut files: Files, version: &Version) -> Result<Files, Error> {
    for path in [PACKAGE_JSON, MANIFEST] {
        if let Some(bytes) = files.get(path) {
            let changed = with_version(path, bytes, version)?;
            files.insert(path.to_owned(), changed);
        }
    }
    Ok(files)
}

/// Whether `path` names a file inside the package that a person may write.
fn writable(path: &str) -> Result<(), Error> {
    let unsafe_path = path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if unsafe_path {
        return Err(Error::Invalid(format!(
            "{path:?} is not a path inside the package"
        )));
    }
    if path == INDEX {
        return Err(Error::Invalid(format!(
            "{INDEX} is generated when the draft is packed"
        )));
    }
    Ok(())
}

impl Publications {
    /// Draft `id`'s files, without the generated index, and its revision.
    pub async fn draft_files(&self, id: &str) -> Result<(Files, u64), Error> {
        let mut tx = self.begin().await?;
        let (lifecycle, revision) = self.load(&mut tx, id).await?;
        let mut files = self.packed(&lifecycle.content).await?.files;
        files.remove(INDEX);
        Ok((files, revision))
    }

    /// `by` writes file `path` of draft `id` (`None` removes it), having read revision
    /// `expected`: the tarball is repacked and becomes the draft's content. Answers the draft's
    /// revision now. A stale `expected` is [`Error::Stale`], and nothing is kept.
    pub async fn save_file(
        &self,
        id: &str,
        path: &str,
        content: Option<Vec<u8>>,
        by: &str,
        expected: u64,
    ) -> Result<u64, Error> {
        writable(path)?;
        let mut tx = self.begin().await?;
        let (mut lifecycle, revision) = self.load(&mut tx, id).await?;
        if revision != expected {
            return Err(Error::Stale { current: revision });
        }
        lifecycle.check(Action::Edit, by)?;
        let mut files = self.packed(&lifecycle.content).await?.files;
        match content {
            Some(bytes) => {
                if path == PACKAGE_JSON {
                    same_package(&lifecycle, &bytes)?;
                }
                files.insert(path.to_owned(), bytes);
            }
            None if path == PACKAGE_JSON || path == MANIFEST => {
                return Err(Error::Invalid(format!("a package keeps its {path}")));
            }
            None => {
                if files.remove(path).is_none() {
                    return Err(Error::NotFound(format!("{id} has no file {path}")));
                }
            }
        }
        let packed = health_packages::pack(&files)?;
        let content = packed.digest.to_string();
        self.put_blob(packed.tgz.clone()).await?;
        if lifecycle.stage == Stage::Draft {
            // In Draft the kind may change with the content; approvals start over anyway.
            lifecycle.kind = packed.contents.package.kind;
        }
        let record = lifecycle.revise(by, &content, self.clock.now())?;
        let records = [(record, format!("saved {path}, {content}"))];
        self.save(tx, &lifecycle, Some(revision), &records)
            .await
            .map_err(|error| stale(error, revision))?;
        Ok(revision + 1)
    }

    /// `by`, the holder, gives draft `id` its one change note, having read revision
    /// `expected`; an empty note clears it. Answers the new revision. The note is not content:
    /// the fingerprint and the history stay as they were.
    pub async fn set_note(
        &self,
        id: &str,
        note: Option<String>,
        by: &str,
        expected: u64,
    ) -> Result<u64, Error> {
        let mut tx = self.begin().await?;
        let (mut lifecycle, revision) = self.load(&mut tx, id).await?;
        if revision != expected {
            return Err(Error::Stale { current: revision });
        }
        lifecycle.set_note(by, note)?;
        self.save(tx, &lifecycle, Some(revision), &[])
            .await
            .map_err(|error| stale(error, revision))?;
        Ok(revision + 1)
    }

    /// `by` gives draft `id` the version `version`, having read revision `expected`: its
    /// `package.json` and manifest take it, and the draft becomes `name@version`, its history
    /// with it. Answers the new id and revision.
    pub async fn change_version(
        &self,
        id: &str,
        version: &str,
        by: &str,
        expected: u64,
    ) -> Result<(String, u64), Error> {
        let version: Version = version
            .parse()
            .map_err(|e| Error::Invalid(format!("the version: {e}")))?;
        let mut tx = self.begin().await?;
        let (mut lifecycle, revision) = self.load(&mut tx, id).await?;
        if revision != expected {
            return Err(Error::Stale { current: revision });
        }
        lifecycle.check(Action::Edit, by)?;
        if version == lifecycle.version {
            return Ok((id.to_owned(), revision));
        }
        let renamed = format!("{}@{version}", lifecycle.package);
        let published = tx.published_package(&renamed).await.map_err(failed)?;
        let drafted = tx.package_lifecycle(&renamed).await.map_err(failed)?;
        if published.is_some() || drafted.is_some() {
            return Err(Error::Conflict(format!(
                "{renamed} is already drafted or published"
            )));
        }
        let mut files = self.packed(&lifecycle.content).await?.files;
        files.remove(INDEX);
        let packed = health_packages::pack(&renumbered(files, &version)?)?;
        let content = packed.digest.to_string();
        self.put_blob(packed.tgz.clone()).await?;
        let previous = lifecycle.version.clone();
        let record = lifecycle.change_version(by, version, &content, self.clock.now())?;
        let data = serde_json::to_string(&lifecycle).map_err(failed)?;
        let moved = tx
            .rekey_package_lifecycle(
                id,
                &renamed,
                &lifecycle.package,
                &lifecycle.stage.to_string(),
                &data,
                revision,
            )
            .await
            .map_err(failed)?;
        if !moved {
            return Err(Error::Stale {
                current: revision + 1,
            });
        }
        let detail = format!("version {previous} → {}, {content}", lifecycle.version);
        self.append(&mut tx, &renamed, &[(record, detail)]).await?;
        tx.commit().await.map_err(failed)?;
        Ok((renamed, revision + 1))
    }

    /// `by` starts a draft of the version after published `id`, a copy of its files: the next
    /// minor, marked `-draft` (`2.0.0` → `2.1.0-draft`), its artefacts as they were.
    pub async fn start_copy(&self, id: &str, by: &str) -> Result<Lifecycle, Error> {
        let mut tx = self.begin().await?;
        let json = tx
            .published_package(id)
            .await
            .map_err(failed)?
            .ok_or_else(|| Error::NotFound(format!("{id} is not published")))?;
        drop(tx);
        let published: PublishedPackage = serde_json::from_str(&json).map_err(failed)?;
        let mut files = self.packed(&published.digest).await?.files;
        files.remove(INDEX);
        let files = renumbered(files, &next_version(&published.version))?;
        self.start(&files, by, &format!("copied from {id}")).await
    }

    /// `by` starts a new package's draft: its `package.json` and a manifest Library, no
    /// artefacts yet.
    pub async fn start_blank(&self, blank: &Blank, by: &str) -> Result<Lifecycle, Error> {
        if !valid_name(&blank.name) {
            return Err(Error::Invalid(format!(
                "{:?} is not a package name: lower-case letters, digits, '.', '-' and '_'",
                blank.name
            )));
        }
        if blank.title.trim().is_empty() {
            return Err(Error::Invalid("a package needs a title".into()));
        }
        let canonical = blank.canonical.trim().trim_end_matches('/');
        if !(canonical.starts_with("http://") || canonical.starts_with("https://")) {
            return Err(Error::Invalid(format!(
                "{canonical:?} is not a canonical base URL"
            )));
        }
        let published = self
            .published(&mut self.begin().await?, &blank.name)
            .await?;
        if !published.is_empty() {
            return Err(Error::Conflict(format!(
                "{} is published already: start from its latest version",
                blank.name
            )));
        }
        let version = blank.version.to_string();
        let format = FORMAT_VERSIONS.iter().max().copied().unwrap_or(1);
        let package = json!({
            "name": blank.name, "version": version, "fhirVersions": [FHIR_VERSION],
            "dependencies": {}, "kind": blank.kind,
            "platform": { "minSoftware": blank.min_software, "formatVersion": format },
        });
        let manifest = json!({
            "resourceType": "Library", "id": "manifest", "title": blank.title.trim(),
            "url": format!("{canonical}/Library/{}-manifest", blank.name), "version": version,
            "status": "active", "type": { "text": "asset-collection" },
        });
        let pretty = |value: &Value| serde_json::to_vec_pretty(value).map_err(failed);
        let files = Files::from([
            (PACKAGE_JSON.to_owned(), pretty(&package)?),
            (MANIFEST.to_owned(), pretty(&manifest)?),
        ]);
        self.start(&files, by, "started blank").await
    }

    /// `by` starts the draft of the package in `files`, which must not exist yet.
    async fn start(&self, files: &Files, by: &str, how: &str) -> Result<Lifecycle, Error> {
        let packed = health_packages::pack(files)?;
        let version: Version = packed
            .contents
            .package
            .version
            .parse()
            .map_err(|e| Error::Invalid(format!("the package version: {e}")))?;
        let content = packed.digest.to_string();
        let mut tx = self.begin().await?;
        let published = tx.published_package(&packed.id).await.map_err(failed)?;
        let drafted = tx.package_lifecycle(&packed.id).await.map_err(failed)?;
        if published.is_some() || drafted.is_some() {
            return Err(Error::Conflict(format!(
                "{} is already drafted or published",
                packed.id
            )));
        }
        self.put_blob(packed.tgz.clone()).await?;
        let (lifecycle, record) = Lifecycle::create(
            &packed.contents.package.name,
            version,
            packed.contents.package.kind,
            by,
            &content,
            self.clock.now(),
        )?;
        let records = [(record, format!("drafted, {how}, {content}"))];
        self.save(tx, &lifecycle, None, &records).await?;
        Ok(lifecycle)
    }

    /// *Check now* on draft `id` by `by`: submission's validation and simulation, their results
    /// kept as the latest, the stage and the revision unchanged.
    pub async fn check_now(&self, id: &str, by: &str) -> Result<Lifecycle, Error> {
        let mut tx = self.begin().await?;
        let (lifecycle, _) = self.load(&mut tx, id).await?;
        lifecycle.check(Action::Check, by)?;
        let packed = self.packed(&lifecycle.content).await?;
        let problems = self.validation(&mut tx, &packed).await?;
        let failures = self.sandbox.simulate(&packed).await;
        let detail = format!(
            "checked: {} validation problem(s), {} failing case(s)",
            problems.len(),
            failures.len()
        );
        let record = lifecycle.checked(by, problems, failures, self.clock.now())?;
        self.append(&mut tx, id, &[(record, detail)]).await?;
        tx.commit().await.map_err(failed)?;
        Ok(lifecycle)
    }
}

/// A `package.json` that keeps draft `lifecycle`'s name and version: the version changes only
/// through [`Publications::change_version`], and the name never.
fn same_package(lifecycle: &Lifecycle, bytes: &[u8]) -> Result<(), Error> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|e| Error::Invalid(format!("{PACKAGE_JSON} does not read: {e}")))?;
    if value["name"].as_str() != Some(lifecycle.package.as_str()) {
        return Err(Error::Invalid(format!(
            "a draft keeps its package name, {}",
            lifecycle.package
        )));
    }
    if value["version"].as_str() != Some(lifecycle.version.to_string().as_str()) {
        return Err(Error::Invalid(
            "change the version with the draft's version, not in package.json".into(),
        ));
    }
    Ok(())
}

/// A lost compare-and-set, as the stale save it is.
fn stale(error: Error, revision: u64) -> Error {
    match error {
        Error::Conflict(_) => Error::Stale {
            current: revision + 1,
        },
        other => other,
    }
}
