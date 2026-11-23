//! Saved Visualizations and Dashboards (analytics.md § Dashboards; ADR 0075; #602).
//!
//! - **A Visualization** is one saved view: a title and its setup (the explorer's question and
//!   how it is drawn), kept as JSON the application layer checks. Its places may *follow the
//!   viewer* (their own place, or each area under it) rather than name places, so one shared
//!   Visualization shows each viewer their own area.
//! - **A Dashboard** is a named arrangement of Visualizations. A *saved* Dashboard is user
//!   content: made by someone holding `analytics.author`, who alone changes it, and shared with
//!   a person, a role or a place level ([`Share`]). *Standard* Dashboards ship in the Reporting
//!   package (#603) and are not kept here.
//! - **Who sees one** ([`SavedDashboard::visible_to`]): its owner, and whoever a share names.
//!   Sharing never widens what a viewer reads: each still sees only their own area.

use std::collections::BTreeSet;
use std::future::Future;

use health_persistence::{StoreError, WriteError};
use serde::{Deserialize, Serialize};

/// Who a saved Dashboard is shared with.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "with", rename_all = "camelCase")]
pub enum Share {
    /// One person, by account id; `username` is kept to show it.
    Person {
        /// Their account id.
        account: String,
        /// Their username, as the owner typed it.
        username: String,
    },
    /// Everyone holding a role, by its code, at whatever place they hold it.
    Role {
        /// The role code.
        role: String,
    },
    /// Everyone whose session is at a place of this level (`facility`, `district`, `region`,
    /// `national`): *every district*.
    Level {
        /// The place level's code.
        level: String,
    },
}

/// Who is looking: what a [`Share`] is matched against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    /// Their account id.
    pub account: String,
    /// The role codes they hold at the session's place.
    pub roles: BTreeSet<String>,
    /// The level of the session's place.
    pub level: String,
}

/// A saved Visualization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedVisualization {
    /// Its id.
    pub id: String,
    /// The account that saved it.
    pub owner: String,
    /// Its title.
    pub title: String,
    /// What it shows and how, as JSON (`{type, question, layout, series, follow?}`).
    pub setup: String,
}

/// A saved Dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedDashboard {
    /// Its id.
    pub id: String,
    /// The account that made it.
    pub owner: String,
    /// Its title.
    pub title: String,
    /// Who it is shared with, besides its owner.
    pub shares: Vec<Share>,
    /// Its Visualizations' ids, in order.
    pub items: Vec<String>,
}

impl SavedDashboard {
    /// Whether `viewer` may open it: its owner, or someone a share names.
    pub fn visible_to(&self, viewer: &Viewer) -> bool {
        self.owner == viewer.account
            || self.shares.iter().any(|share| match share {
                Share::Person { account, .. } => *account == viewer.account,
                Share::Role { role } => viewer.roles.contains(role),
                Share::Level { level } => *level == viewer.level,
            })
    }
}

/// The saved Visualizations and Dashboards central keeps, over a transaction.
pub trait SavedDashboards {
    /// Keeps `visualization`, replacing one with its id.
    fn put_visualization(
        &mut self,
        visualization: &SavedVisualization,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Every saved Visualization.
    fn visualizations(
        &mut self,
    ) -> impl Future<Output = Result<Vec<SavedVisualization>, StoreError>> + Send;

    /// Keeps `dashboard`, replacing one with its id.
    fn put_dashboard(
        &mut self,
        dashboard: &SavedDashboard,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Every saved Dashboard, oldest first.
    fn dashboards(
        &mut self,
    ) -> impl Future<Output = Result<Vec<SavedDashboard>, StoreError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer(account: &str, roles: &[&str], level: &str) -> Viewer {
        Viewer {
            account: account.into(),
            roles: roles.iter().map(|r| (*r).to_owned()).collect(),
            level: level.into(),
        }
    }

    fn shared(shares: Vec<Share>) -> SavedDashboard {
        SavedDashboard {
            id: "d1".into(),
            owner: "okot".into(),
            title: "ANC".into(),
            shares,
            items: Vec::new(),
        }
    }

    #[test]
    fn its_owner_and_whoever_a_share_names_see_it() {
        let wakiso = viewer("nakato", &["district"], "district");
        let facility = viewer("akello", &["district"], "facility");
        assert!(shared(Vec::new()).visible_to(&viewer("okot", &[], "district")));
        assert!(!shared(Vec::new()).visible_to(&wakiso));

        let every_district = shared(vec![Share::Level {
            level: "district".into(),
        }]);
        assert!(every_district.visible_to(&wakiso));
        assert!(!every_district.visible_to(&facility));

        let role = shared(vec![Share::Role {
            role: "district".into(),
        }]);
        assert!(role.visible_to(&facility));
        assert!(!role.visible_to(&viewer("opio", &["clerk"], "facility")));

        let person = shared(vec![Share::Person {
            account: "nakato".into(),
            username: "e2e-wakiso".into(),
        }]);
        assert!(person.visible_to(&wakiso));
        assert!(!person.visible_to(&facility));
    }

    #[test]
    fn a_share_reads_and_writes_as_tagged_json() {
        let share = Share::Level {
            level: "district".into(),
        };
        let json = serde_json::to_value(&share).expect("json");
        assert_eq!(
            json,
            serde_json::json!({ "with": "level", "level": "district" })
        );
        assert_eq!(serde_json::from_value::<Share>(json).expect("share"), share);
    }
}
