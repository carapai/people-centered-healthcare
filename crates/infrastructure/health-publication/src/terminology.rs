//! The terminology a package version is authored against, for the Studio's terminology browser
//! (studio.md § Terminology; #523).
//!
//! - **What it sees:** its own CodeSystems and ValueSets as they are now, and the latest
//!   published version of every other package. A code system from a `terminology` package is
//!   external; the rest are local.
//! - **The change against published** compares a ValueSet's expansion now with its expansion in
//!   the package's previous published version, both over the other packages as published.
//! - **Published content never changes**, so what is read from a published tarball is kept by
//!   its digest and not read again.
//! - **Editing a local CodeSystem's concepts** (#524): [`Publications::draft_code_system`] reads
//!   a draft's CodeSystem with its previous published version, which fixes the codes it holds,
//!   and [`Publications::edit_concept`] applies one edit (`health_packages::concepts`), gives the
//!   CodeSystem the version the change calls for, and saves it under the draft's revision check.
//!   A note on the edit goes to the draft's discussion, naming the CodeSystem. A `terminology`
//!   package's code systems are external, and stay read only.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use health_configuration::lifecycle::{Lifecycle, Stage};
use health_configuration::package::PackageKind;
use health_packages::concepts::{self, ConceptEdit};
use health_packages::terminology::{Catalogue, catalogue, load};
use health_terminology::Terminology;
use serde_json::Value;

use crate::{Error, Publications, PublishedPackage, failed, lifecycle_id};

/// A package version's terminology, read once per digest.
#[derive(Debug, Clone)]
pub struct Part {
    /// `name@version`.
    pub id: String,
    /// The package name.
    pub package: String,
    /// Its kind: a `terminology` package's code systems are external.
    pub kind: PackageKind,
    /// Whether it is published (and so read only).
    pub published: bool,
    /// Its CodeSystems, ValueSets and ConceptMaps.
    pub terminology: Arc<Terminology>,
    /// Its CodeSystems, ValueSets and bindings, by file.
    pub catalogue: Arc<Catalogue>,
}

/// What a package version is authored against.
#[derive(Debug, Clone)]
pub struct StudioTerminology {
    /// The package version itself, as it is now.
    pub own: Part,
    /// Its package's latest published version before it, if any.
    pub previous: Option<Part>,
    /// The latest published version of every other package, by name.
    pub others: Vec<Part>,
}

impl StudioTerminology {
    fn over(&self, top: Option<&Part>) -> Terminology {
        let mut terminology = Terminology::new();
        for part in self.others.iter().chain(top) {
            terminology.extend(&part.terminology);
        }
        terminology
    }

    /// The other packages as published, with this package version over them.
    pub fn current(&self) -> Terminology {
        self.over(Some(&self.own))
    }

    /// The other packages as published, with this package's previous published version over
    /// them: what its ValueSets expanded to when last published.
    pub fn published(&self) -> Terminology {
        self.over(self.previous.as_ref())
    }

    /// Every part, this package version first.
    pub fn parts(&self) -> impl Iterator<Item = &Part> {
        std::iter::once(&self.own).chain(&self.others)
    }
}

/// What has been read from published tarballs, by digest.
pub(crate) type Read = Arc<Mutex<HashMap<String, (Arc<Terminology>, Arc<Catalogue>)>>>;

fn published(p: &PublishedPackage) -> Of<'_> {
    Of {
        id: p.id.clone(),
        package: &p.package,
        kind: p.kind,
        published: true,
        digest: &p.digest,
    }
}

/// Which package version a part is read from.
struct Of<'a> {
    id: String,
    package: &'a str,
    kind: PackageKind,
    published: bool,
    digest: &'a str,
}

impl Publications {
    async fn part(&self, of: Of<'_>) -> Result<Part, Error> {
        let kept = self
            .read
            .lock()
            .map_err(|_| failed("the terminology cache is poisoned"))?
            .get(of.digest)
            .cloned();
        let (terminology, catalogue) = match kept {
            Some(kept) => kept,
            None => {
                let packed = self.packed(of.digest).await?;
                let mut terminology = Terminology::new();
                load(&mut terminology, &packed.files)?;
                let read = (Arc::new(terminology), Arc::new(catalogue(&packed.files)));
                if of.published {
                    self.read
                        .lock()
                        .map_err(|_| failed("the terminology cache is poisoned"))?
                        .insert(of.digest.to_owned(), read.clone());
                }
                read
            }
        };
        Ok(Part {
            id: of.id,
            package: of.package.to_owned(),
            kind: of.kind,
            published: of.published,
            terminology,
            catalogue,
        })
    }

    /// What package version `id` (`name@version`) is authored against.
    pub async fn studio_terminology(&self, id: &str) -> Result<StudioTerminology, Error> {
        let (lifecycle, _) = self.lifecycle(id).await?;
        let mut tx = self.begin().await?;
        let previous = self.previous(&mut tx, &lifecycle).await?;
        let mut names = BTreeSet::new();
        for json in tx.package_lifecycles().await.map_err(failed)? {
            let other: Lifecycle = serde_json::from_str(&json).map_err(failed)?;
            if other.package != lifecycle.package {
                names.insert(other.package);
            }
        }
        let mut latest = Vec::new();
        for name in names {
            let newest = self
                .published(&mut tx, &name)
                .await?
                .into_iter()
                .max_by(|a, b| a.version.cmp(&b.version));
            latest.extend(newest);
        }
        drop(tx);
        let own = self
            .part(Of {
                id: lifecycle_id(&lifecycle),
                package: &lifecycle.package,
                kind: lifecycle.kind,
                published: lifecycle.stage == Stage::Published,
                digest: &lifecycle.content,
            })
            .await?;
        let previous = match &previous {
            Some(p) => Some(self.part(published(p)).await?),
            None => None,
        };
        let mut others = Vec::with_capacity(latest.len());
        for p in &latest {
            others.push(self.part(published(p)).await?);
        }
        Ok(StudioTerminology {
            own,
            previous,
            others,
        })
    }
}

/// A draft's own CodeSystem, as the Studio edits it (#524).
#[derive(Debug, Clone, PartialEq)]
pub struct DraftCodeSystem {
    /// Its file in the draft.
    pub path: String,
    /// The resource as it is now.
    pub current: Value,
    /// The same CodeSystem (by URL) in the package's previous published version, if any: the
    /// codes it holds are fixed.
    pub previous: Option<Value>,
    /// The draft's revision it was read at.
    pub revision: u64,
}

impl DraftCodeSystem {
    /// The codes that are published, and so fixed.
    pub fn published(&self) -> BTreeSet<String> {
        self.previous
            .as_ref()
            .map(concepts::codes)
            .unwrap_or_default()
    }
}

/// What an edit did, in a few words, to head its note in the discussion.
fn summary(edit: &ConceptEdit) -> String {
    match edit {
        ConceptEdit::Add { code, parent, .. } => match parent {
            Some(parent) => format!("Added code {code} under {parent}"),
            None => format!("Added code {code}"),
        },
        ConceptEdit::Change { code, new_code, .. } if code != new_code => {
            format!("Changed code {code} to {new_code}")
        }
        ConceptEdit::Change { code, .. } => format!("Changed code {code}"),
        ConceptEdit::Delete { code } => format!("Deleted code {code}"),
        ConceptEdit::Retire { code } => format!("Retired code {code}"),
    }
}

impl Publications {
    /// Draft `id`'s CodeSystem at `path`, with its previous published version. A `terminology`
    /// package's code systems are external: they are not edited here.
    pub async fn draft_code_system(&self, id: &str, path: &str) -> Result<DraftCodeSystem, Error> {
        let mut tx = self.begin().await?;
        let (lifecycle, revision) = self.load(&mut tx, id).await?;
        if lifecycle.kind == PackageKind::Terminology {
            return Err(Error::Invalid(format!(
                "{id} is a terminology package: its code systems are external and read only"
            )));
        }
        let previous = self.previous(&mut tx, &lifecycle).await?;
        drop(tx);
        let files = self.packed(&lifecycle.content).await?.files;
        let bytes = files
            .get(path)
            .ok_or_else(|| Error::NotFound(format!("{id} has no file {path}")))?;
        let current: Value = serde_json::from_slice(bytes)
            .map_err(|e| Error::Invalid(format!("{path} does not read: {e}")))?;
        if current.get("resourceType").and_then(Value::as_str) != Some("CodeSystem") {
            return Err(Error::Invalid(format!("{path} is not a CodeSystem")));
        }
        let url = current.get("url").and_then(Value::as_str);
        let previous = match previous {
            Some(published) => self
                .packed(&published.digest)
                .await?
                .files
                .iter()
                .filter(|(path, _)| !path.contains('/') && path.ends_with(".json"))
                .filter_map(|(_, bytes)| serde_json::from_slice::<Value>(bytes).ok())
                .find(|value| {
                    value.get("resourceType").and_then(Value::as_str) == Some("CodeSystem")
                        && value.get("url").and_then(Value::as_str) == url
                }),
            None => None,
        };
        Ok(DraftCodeSystem {
            path: path.to_owned(),
            current,
            previous,
            revision,
        })
    }

    /// `by` applies `edit` to draft `id`'s CodeSystem at `path`, having read revision
    /// `expected`: a published code is only ever retired, and the CodeSystem takes the version
    /// the change calls for against its published one. A `note`, if any, joins the draft's
    /// discussion naming the CodeSystem, for reviewers. Answers the draft's revision now.
    pub async fn edit_concept(
        &self,
        id: &str,
        path: &str,
        edit: &ConceptEdit,
        note: Option<&str>,
        by: &str,
        expected: u64,
    ) -> Result<u64, Error> {
        let draft = self.draft_code_system(id, path).await?;
        if draft.revision != expected {
            return Err(Error::Stale {
                current: draft.revision,
            });
        }
        let edited = concepts::apply(
            &draft.current,
            edit,
            &draft.published(),
            draft.previous.as_ref(),
        )
        .map_err(|refused| Error::Invalid(refused.to_string()))?;
        let mut bytes = serde_json::to_vec_pretty(&edited).map_err(failed)?;
        bytes.push(b'\n');
        let revision = self.save_file(id, path, Some(bytes), by, expected).await?;
        if let Some(note) = note.map(str::trim).filter(|n| !n.is_empty()) {
            self.comment(id, by, &format!("{}: {note}", summary(edit)), Some(path))
                .await?;
        }
        Ok(revision)
    }
}
