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

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use health_configuration::lifecycle::{Lifecycle, Stage};
use health_configuration::package::PackageKind;
use health_packages::terminology::{Catalogue, catalogue, load};
use health_terminology::Terminology;

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
