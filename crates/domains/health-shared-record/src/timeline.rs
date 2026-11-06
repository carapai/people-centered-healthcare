//! The longitudinal patient timeline (care-journeys.md; MASTER_PROMPT §105; #115): an authorized
//! activity timeline across the person's Encounters, EpisodesOfCare, Observations, orders,
//! medications, results, appointments, notes and Tasks, the newest first.
//!
//! It is a **read model**. Each entry names its authoritative resource and that resource's
//! version; the timeline owns nothing and never edits what it shows (resource-ownership.md rule 8).
//!
//! It is a fold over resource versions, so it can be maintained from events
//! ([`Timeline::apply`]) or rebuilt from current resources ([`Timeline::rebuild`]), with the same
//! result:
//! - a version replaces the entry only if it is newer than the one held, so versions arriving out
//!   of order, or twice, change nothing;
//! - a version that leaves the timeline (deleted, or entered in error) is remembered by its
//!   version, so an older one arriving later does not bring the entry back;
//! - [`Timeline::entries`] orders by time, newest first, then by reference, whatever the order
//!   things arrived in.
//!
//! Versions are compared as text: FHIR version ids here are UUIDv7 operation ids (ADR 0013), whose
//! text orders them by time. Times are FHIR `dateTime` text in UTC, so their text orders them too.

use std::collections::BTreeMap;

use serde::Serialize;

/// What kind of activity an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A visit (Encounter).
    Visit,
    /// Time in a program (EpisodeOfCare).
    Program,
    /// A measurement or finding (Observation).
    Observation,
    /// An order or referral (ServiceRequest).
    Order,
    /// A prescription (MedicationRequest).
    Medication,
    /// A result (DiagnosticReport).
    Result,
    /// An appointment.
    Appointment,
    /// A clinical note (Composition).
    Note,
    /// A unit of work (Task), whose state is workflow state, not clinical status (ADR 0042).
    Task,
}

/// One activity on the timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The authoritative resource, `Type/id`.
    pub reference: String,
    /// The version shown, its `meta.versionId`.
    pub version: String,
    /// What kind of activity.
    pub kind: Kind,
    /// When it happened, or was made: FHIR `dateTime` text in UTC.
    pub at: String,
    /// What a person reads.
    pub title: String,
    /// Its status code, as its resource has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The episode of care it belongs to, `EpisodeOfCare/{id}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    /// The facility whose record it is (its Authority), from its facility tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facility: Option<String>,
}

/// A version of a resource, as the timeline sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The version is on the timeline as `Entry`.
    Shown(Entry),
    /// The version leaves the timeline: `reference` at `version` was deleted, entered in error, or
    /// is no longer something the timeline shows.
    Gone {
        /// `Type/id`.
        reference: String,
        /// Its `meta.versionId`.
        version: String,
    },
}

impl Change {
    fn reference(&self) -> &str {
        match self {
            Self::Shown(entry) => &entry.reference,
            Self::Gone { reference, .. } => reference,
        }
    }

    fn version(&self) -> &str {
        match self {
            Self::Shown(entry) => &entry.version,
            Self::Gone { version, .. } => version,
        }
    }
}

/// The timeline of one person.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Timeline {
    /// The latest version seen of each resource, shown or gone.
    latest: BTreeMap<String, Change>,
}

impl Timeline {
    /// An empty timeline.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The timeline of these versions, the current version of each resource.
    pub fn rebuild(current: impl IntoIterator<Item = Change>) -> Self {
        let mut timeline = Self::new();
        for change in current {
            timeline.apply(change);
        }
        timeline
    }

    /// Takes in one version. An older version than the one held, or the same one again, changes
    /// nothing.
    pub fn apply(&mut self, change: Change) {
        let newer = self
            .latest
            .get(change.reference())
            .is_none_or(|held| change.version() > held.version());
        if newer {
            self.latest.insert(change.reference().to_owned(), change);
        }
    }

    /// The entries, newest first, then by reference.
    #[must_use]
    pub fn entries(&self) -> Vec<&Entry> {
        let mut shown: Vec<&Entry> = self
            .latest
            .values()
            .filter_map(|change| match change {
                Change::Shown(entry) => Some(entry),
                Change::Gone { .. } => None,
            })
            .collect();
        shown.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.reference.cmp(&b.reference)));
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(reference: &str, version: &str, at: &str, status: &str) -> Change {
        Change::Shown(Entry {
            reference: reference.into(),
            version: version.into(),
            kind: Kind::Visit,
            at: at.into(),
            title: format!("{reference} title"),
            status: Some(status.into()),
            episode: None,
            facility: Some("KAM013".into()),
        })
    }

    fn gone(reference: &str, version: &str) -> Change {
        Change::Gone {
            reference: reference.into(),
            version: version.into(),
        }
    }

    /// Every version each resource went through, in the order they were written.
    fn history() -> Vec<Change> {
        vec![
            entry("Encounter/a", "v01", "2026-11-01T08:00:00Z", "planned"),
            entry("Encounter/b", "v02", "2026-11-02T08:00:00Z", "in-progress"),
            entry("Encounter/a", "v03", "2026-11-01T08:00:00Z", "completed"),
            entry("Observation/c", "v04", "2026-11-02T08:00:00Z", "final"),
            entry("Observation/d", "v05", "2026-11-03T08:00:00Z", "final"),
            gone("Observation/d", "v06"),
            entry("Encounter/b", "v07", "2026-11-02T08:00:00Z", "completed"),
        ]
    }

    /// The current version of each resource: what a rebuild reads.
    fn current() -> Vec<Change> {
        let mut latest = BTreeMap::new();
        for change in history() {
            latest.insert(change.reference().to_owned(), change);
        }
        latest.into_values().collect()
    }

    /// A deterministic shuffle (a linear congruential generator), so the test needs no crates.
    fn shuffled(mut changes: Vec<Change>, seed: u64) -> Vec<Change> {
        let mut state = seed;
        for i in (1..changes.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = usize::try_from(state >> 33).unwrap_or(0) % (i + 1);
            changes.swap(i, j);
        }
        changes
    }

    #[test]
    fn a_rebuild_equals_the_timeline_kept_from_events_in_any_order() {
        let rebuilt = Timeline::rebuild(current());
        for seed in 0..200 {
            let mut kept = Timeline::new();
            let mut events = shuffled(history(), seed);
            // Some events arrive twice.
            events.extend(shuffled(history(), seed + 1000).into_iter().take(3));
            for change in events {
                kept.apply(change);
            }
            assert_eq!(kept.entries(), rebuilt.entries(), "seed {seed}");
        }
        let shown: Vec<(&str, Option<&str>)> = rebuilt
            .entries()
            .iter()
            .map(|e| (e.reference.as_str(), e.status.as_deref()))
            .collect();
        assert_eq!(
            shown,
            [
                ("Encounter/b", Some("completed")),
                ("Observation/c", Some("final")),
                ("Encounter/a", Some("completed")),
            ],
            "newest first, then by reference; the entered-in-error observation is gone"
        );
    }

    #[test]
    fn an_older_version_never_brings_an_entry_back() {
        let mut timeline = Timeline::new();
        timeline.apply(gone("Observation/d", "v06"));
        timeline.apply(entry(
            "Observation/d",
            "v05",
            "2026-11-03T08:00:00Z",
            "final",
        ));
        assert!(timeline.entries().is_empty());
        timeline.apply(entry(
            "Observation/d",
            "v08",
            "2026-11-03T08:00:00Z",
            "amended",
        ));
        assert_eq!(
            timeline.entries()[0].status.as_deref(),
            Some("amended"),
            "a newer version does"
        );
    }
}
