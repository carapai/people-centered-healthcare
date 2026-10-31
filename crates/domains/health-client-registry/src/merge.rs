use crate::golden::SourceRecord;

/// Why two sources cannot be merged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MergeRefused {
    /// Only records of the same facility are merged; across facilities they are linked.
    #[error("the records belong to different facilities; link them instead")]
    DifferentFacility,
    /// A record cannot be merged into itself.
    #[error("a record cannot be merged into itself")]
    SameRecord,
    /// One of them is already merged away.
    #[error("one of the records is inactive")]
    Inactive,
    /// The plan was not approved by a person.
    #[error("a merge needs a person's approval after the preview")]
    NotApproved,
}

/// A previewed `$merge` (ADR 0027): `source` becomes inactive and `replaced-by` `target`. No
/// reference to `source` is rewritten, so every record that points at it stays valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePlan {
    /// The duplicate, deactivated.
    pub source: String,
    /// The record that remains.
    pub target: String,
    /// Their facility.
    pub facility: String,
    /// Who approved it after the preview, once someone has.
    pub approved_by: Option<String>,
}

impl MergePlan {
    /// The plan with a person's approval.
    #[must_use]
    pub fn approve(mut self, reviewer: &str) -> Self {
        self.approved_by = Some(reviewer.to_owned());
        self
    }

    /// Checks the plan is approved before it is carried out; nothing is ever merged
    /// automatically.
    pub fn ready(&self) -> Result<&str, MergeRefused> {
        self.approved_by
            .as_deref()
            .filter(|r| !r.trim().is_empty())
            .ok_or(MergeRefused::NotApproved)
    }
}

/// Plans merging `source` into `target`, for preview.
pub fn plan_merge(source: &SourceRecord, target: &SourceRecord) -> Result<MergePlan, MergeRefused> {
    if source.id == target.id {
        return Err(MergeRefused::SameRecord);
    }
    if !source.active || !target.active {
        return Err(MergeRefused::Inactive);
    }
    match (&source.facility, &target.facility) {
        (Some(a), Some(b)) if a == b => Ok(MergePlan {
            source: source.id.clone(),
            target: target.id.clone(),
            facility: a.clone(),
            approved_by: None,
        }),
        _ => Err(MergeRefused::DifferentFacility),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, facility: Option<&str>) -> SourceRecord {
        SourceRecord {
            id: id.into(),
            facility: facility.map(str::to_owned),
            active: true,
            claims: vec![],
        }
    }

    #[test]
    fn only_same_facility_records_merge_and_only_with_approval() {
        let plan = plan_merge(&record("a", Some("F1")), &record("b", Some("F1"))).expect("plan");
        assert_eq!(plan.ready(), Err(MergeRefused::NotApproved));
        assert_eq!(
            plan.clone().approve(" ").ready(),
            Err(MergeRefused::NotApproved)
        );
        assert_eq!(
            plan.approve("Practitioner/r1").ready(),
            Ok("Practitioner/r1")
        );
        assert_eq!(
            plan_merge(&record("a", Some("F1")), &record("b", Some("F2"))),
            Err(MergeRefused::DifferentFacility)
        );
        assert_eq!(
            plan_merge(&record("a", None), &record("b", None)),
            Err(MergeRefused::DifferentFacility)
        );
        assert_eq!(
            plan_merge(&record("a", Some("F1")), &record("a", Some("F1"))),
            Err(MergeRefused::SameRecord)
        );
        let mut gone = record("b", Some("F1"));
        gone.active = false;
        assert_eq!(
            plan_merge(&record("a", Some("F1")), &gone),
            Err(MergeRefused::Inactive)
        );
    }
}
