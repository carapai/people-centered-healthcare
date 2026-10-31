use std::collections::{BTreeMap, BTreeSet};

use crate::ecid::Ecid;
use crate::survivorship::{Claim, Field, ReviewItem, Survival, survive};

/// A source Patient as the registry sees it: its id, facility, whether it is active, and the
/// values it claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    /// The source Patient's id.
    pub id: String,
    /// The facility that registered it (its managing organization).
    pub facility: Option<String>,
    /// False once merged into another source.
    pub active: bool,
    /// Its values, one per field at most.
    pub claims: Vec<(Field, Claim)>,
}

/// Why a source cannot be linked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkRefused {
    /// A reviewer unlinked this pair: matching must not re-link it.
    #[error("a reviewer separated this source from this record")]
    Adjudicated,
    /// Already linked here.
    #[error("the source is already linked to this record")]
    AlreadyLinked,
    /// Merged sources are not linked; their replacement is.
    #[error("the source is inactive (merged)")]
    Inactive,
    /// Not linked here.
    #[error("the source is not linked to this record")]
    NotLinked,
}

/// The golden record (ADR 0027): the ECID, the linked sources, the surviving value of each
/// field, and the sources reviewers have separated from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenRecord {
    /// The platform health id.
    pub ecid: Ecid,
    /// The linked source ids.
    pub sources: BTreeSet<String>,
    /// The surviving value of each field.
    pub claims: BTreeMap<Field, Claim>,
    /// Sources that must never be re-linked here (adjudication markers).
    pub separated: BTreeSet<String>,
}

impl GoldenRecord {
    /// A new golden record with no sources.
    pub fn new(ecid: Ecid) -> Self {
        Self {
            ecid,
            sources: BTreeSet::new(),
            claims: BTreeMap::new(),
            separated: BTreeSet::new(),
        }
    }

    /// Applies one source's values by survivorship, returning the disagreements to review.
    fn absorb(&mut self, source: &SourceRecord) -> Vec<ReviewItem> {
        let mut review = Vec::new();
        for (field, claim) in &source.claims {
            match survive(*field, self.claims.get(field), claim) {
                Survival::Adopted => {
                    self.claims.insert(*field, claim.clone());
                }
                Survival::Kept => {}
                Survival::Review(item) => review.push(item),
            }
        }
        review
    }

    /// Links `source` (auto-link or a reviewer's decision) and takes its values by
    /// survivorship. Returns the values that need review.
    pub fn link(&mut self, source: &SourceRecord) -> Result<Vec<ReviewItem>, LinkRefused> {
        if self.separated.contains(&source.id) {
            return Err(LinkRefused::Adjudicated);
        }
        if !source.active {
            return Err(LinkRefused::Inactive);
        }
        if !self.sources.insert(source.id.clone()) {
            return Err(LinkRefused::AlreadyLinked);
        }
        Ok(self.absorb(source))
    }

    /// A source's values changed (a newer version): survivorship again.
    pub fn update(&mut self, source: &SourceRecord) -> Result<Vec<ReviewItem>, LinkRefused> {
        if !self.sources.contains(&source.id) {
            return Err(LinkRefused::NotLinked);
        }
        Ok(self.absorb(source))
    }

    /// A reviewer separates `source_id`: it leaves, an adjudication marker stops it being
    /// re-linked, and the surviving values are worked out again from `remaining` (the sources
    /// still linked), so nothing of the separated source survives.
    pub fn unlink(
        &mut self,
        source_id: &str,
        remaining: &[SourceRecord],
    ) -> Result<(), LinkRefused> {
        if !self.sources.remove(source_id) {
            return Err(LinkRefused::NotLinked);
        }
        self.separated.insert(source_id.to_owned());
        self.claims.clear();
        // Oldest first, so recency within a level ends as it would have.
        let mut claims: Vec<(Field, Claim)> = remaining
            .iter()
            .filter(|s| self.sources.contains(&s.id))
            .flat_map(|s| s.claims.iter().cloned())
            .collect();
        claims.sort_by_key(|(_, c)| c.recorded);
        for (field, claim) in claims {
            if survive(field, self.claims.get(&field), &claim) == Survival::Adopted {
                self.claims.insert(field, claim);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use health_time::Timestamp;

    use super::*;
    use crate::survivorship::Trust;

    fn source(id: &str, claims: &[(Field, &str, Trust, i64)]) -> SourceRecord {
        SourceRecord {
            id: id.into(),
            facility: Some("F1".into()),
            active: true,
            claims: claims
                .iter()
                .map(|(f, v, t, at)| {
                    (
                        *f,
                        Claim {
                            value: (*v).into(),
                            trust: *t,
                            recorded: Timestamp::from_second(*at).unwrap_or(Timestamp::UNIX_EPOCH),
                            source: id.into(),
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn linking_combines_sources_by_trust() {
        let mut golden = GoldenRecord::new(Ecid::from_random(7));
        let a = source(
            "a",
            &[
                (Field::Family, "Okello", Trust::StaffEntered, 10),
                (Field::Nin, "CM86A", Trust::NiraVerified, 10),
            ],
        );
        let b = source(
            "b",
            &[
                (Field::Family, "Okelo", Trust::StaffVerified, 5),
                (Field::Nin, "CM86B", Trust::SelfReported, 20),
                (Field::Phone, "+256700000001", Trust::SelfReported, 20),
            ],
        );
        assert_eq!(golden.link(&a), Ok(vec![]));
        let review = golden.link(&b).expect("link");
        assert_eq!(
            golden.claims[&Field::Family].value,
            "Okelo",
            "staff-verified beats staff-entered"
        );
        assert_eq!(
            golden.claims[&Field::Nin].value,
            "CM86A",
            "self-reported never overwrites NIRA-verified"
        );
        assert_eq!(golden.claims[&Field::Phone].value, "+256700000001");
        assert_eq!(review.len(), 1);
        assert_eq!(review[0].field, Field::Nin);
        assert_eq!(golden.link(&b), Err(LinkRefused::AlreadyLinked));
    }

    #[test]
    fn unlinking_splits_and_is_never_undone_by_matching() {
        let mut golden = GoldenRecord::new(Ecid::from_random(7));
        let a = source("a", &[(Field::Family, "Okello", Trust::StaffEntered, 10)]);
        let b = source(
            "b",
            &[
                (Field::Family, "Akello", Trust::StaffVerified, 20),
                (Field::Gender, "female", Trust::StaffVerified, 20),
            ],
        );
        golden.link(&a).expect("a");
        golden.link(&b).expect("b");
        assert_eq!(golden.claims[&Field::Family].value, "Akello");
        golden.unlink("b", &[a.clone(), b.clone()]).expect("unlink");
        assert_eq!(
            golden.claims[&Field::Family].value,
            "Okello",
            "b's values leave with it"
        );
        assert!(!golden.claims.contains_key(&Field::Gender));
        assert_eq!(golden.link(&b), Err(LinkRefused::Adjudicated));
        assert_eq!(golden.unlink("b", &[]), Err(LinkRefused::NotLinked));
        let mut merged = source("c", &[]);
        merged.active = false;
        assert_eq!(golden.link(&merged), Err(LinkRefused::Inactive));
        assert_eq!(golden.update(&merged), Err(LinkRefused::NotLinked));
        assert_eq!(golden.update(&a).map(|r| r.len()), Ok(0));
    }
}
