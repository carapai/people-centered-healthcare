//! Saved Visualizations and Dashboards (analytics.md § Dashboards; ADR 0075; #602): what a save
//! or a change of sharing may hold, and how a Dashboard reads to a viewer.
//!
//! - **A Visualization's setup** ([`read_setup`]) is the explorer's: a question it could ask
//!   ([`read_question`] over the indicators offered), its type, table layout and series, and
//!   optionally `follow`: [`MY_PLACE`] or [`EACH_AREA_UNDER`], so the places are the viewer's
//!   own rather than the author's. Whether the author may see the places is the caller's to
//!   check.
//! - **Shares** ([`read_shares`]) name a person by username, a role by code, or a place level;
//!   the caller turns usernames into accounts.
//! - **A Dashboard as a viewer reads it** ([`dashboard_json`]): its Visualizations in order, with
//!   whether it is the viewer's own. Its shares are listed only to its owner.
//! - **A standard Dashboard** (#603; [`standard_json`]) reads the same, from the Reporting
//!   package that ships it: never the viewer's own, its periods as of today, its places the
//!   viewer's. *Save as* copies its Visualizations as the author sees them ([`copied_setup`]).

use std::collections::BTreeSet;

use health_analytics::{
    Indicator, SavedDashboard, SavedVisualization, Share, StandardDashboard, StandardItem, Viewer,
};
use jiff::civil::Date;
use serde_json::{Value, json};

use crate::explore::read_question;

/// The use-case permission that saves Visualizations and Dashboards (ADR 0075).
pub const AUTHOR_PERMISSION: &str = "analytics.author";

pub use health_analytics::standard::{
    EACH_AREA_UNDER, LAYOUTS, MOST_ITEMS, MY_PLACE, SERIES, TYPES,
};

/// The longest title kept.
pub const MOST_TITLE: usize = 120;

/// The most shares a Dashboard holds.
pub const MOST_SHARES: usize = 50;

/// `body[name]` as a title: trimmed, not empty, at most [`MOST_TITLE`] characters.
pub fn read_title(body: &Value, name: &str) -> Result<String, String> {
    let title = body[name]
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or(format!("give `{name}`"))?;
    if title.chars().count() > MOST_TITLE {
        return Err(format!("`{name}` is longer than {MOST_TITLE} characters"));
    }
    Ok(title.to_owned())
}

/// A Visualization's setup as saved, and the places its question names.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    /// `{type, question, layout, series, follow?}`, holding only those.
    pub json: Value,
    /// The places its question names.
    pub places: Vec<String>,
    /// Whose places it shows: [`MY_PLACE`], [`EACH_AREA_UNDER`], or none for the places named.
    pub follow: Option<String>,
}

/// Reads `body` as a Visualization's setup over the indicators `offered`, or says why it cannot
/// be saved.
pub fn read_setup(body: &Value, offered: &[Indicator]) -> Result<Setup, String> {
    let one_of = |name: &str, allowed: &[&str]| -> Result<String, String> {
        body[name]
            .as_str()
            .filter(|v| allowed.contains(v))
            .map(str::to_owned)
            .ok_or(format!("`{name}` is not one of {}", allowed.join(", ")))
    };
    let kind = one_of("type", &TYPES)?;
    let layout = one_of("layout", &LAYOUTS)?;
    let series = one_of("series", &SERIES)?;
    let question = &body["question"];
    let read = read_question(question, offered)?;
    if series == "band" && read.disaggregation.is_none() {
        return Err("series by band need a disaggregation".into());
    }
    let follow = match &body["follow"] {
        Value::Null => None,
        Value::String(f) if f == MY_PLACE || f == EACH_AREA_UNDER => Some(f.clone()),
        _ => {
            return Err(format!(
                "`follow` is not one of {MY_PLACE}, {EACH_AREA_UNDER}"
            ));
        }
    };
    let mut kept = json!({
        "indicators": read.indicators.iter().map(Indicator::canonical).collect::<Vec<_>>(),
        "periodType": question["periodType"],
        "from": question["from"],
        "to": question["to"],
        "places": read.places,
    });
    if let Some(name) = &read.disaggregation {
        kept["disaggregation"] = json!(name);
    }
    let mut json = json!({ "type": kind, "question": kept, "layout": layout, "series": series });
    if let Some(follow) = &follow {
        json["follow"] = json!(follow);
    }
    Ok(Setup {
        json,
        places: read.places,
        follow,
    })
}

/// Shares as asked: those that need nothing more, and the usernames of the people named.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AskedShares {
    /// Roles and place levels.
    pub shares: Vec<Share>,
    /// The people, by username, for the caller to find.
    pub people: Vec<String>,
}

/// Reads `body[name]` as a list of shares (`{with: person, username}`, `{with: role, role}`,
/// `{with: level, level}`), each place level one of `levels`.
pub fn read_shares(body: &Value, name: &str, levels: &[&str]) -> Result<AskedShares, String> {
    let list = match &body[name] {
        Value::Null => return Ok(AskedShares::default()),
        Value::Array(list) => list,
        _ => return Err(format!("`{name}` is not a list")),
    };
    if list.len() > MOST_SHARES {
        return Err(format!("share with at most {MOST_SHARES}"));
    }
    let mut asked = AskedShares::default();
    let mut seen = BTreeSet::new();
    for share in list {
        let text = |field: &str| {
            share[field]
                .as_str()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .ok_or(format!("a share with {} needs `{field}`", share["with"]))
        };
        match share["with"].as_str() {
            Some("person") => {
                let username = text("username")?;
                if seen.insert(format!("person {username}")) {
                    asked.people.push(username);
                }
            }
            Some("role") => {
                let role = text("role")?;
                if seen.insert(format!("role {role}")) {
                    asked.shares.push(Share::Role { role });
                }
            }
            Some("level") => {
                let level = text("level")?;
                if !levels.contains(&level.as_str()) {
                    return Err(format!("{level} is not a place level"));
                }
                if seen.insert(format!("level {level}")) {
                    asked.shares.push(Share::Level { level });
                }
            }
            _ => return Err("a share is with a person, a role or a level".into()),
        }
    }
    Ok(asked)
}

fn visualization_json(visualization: &SavedVisualization) -> Value {
    json!({
        "id": visualization.id,
        "title": visualization.title,
        "visualization": serde_json::from_str::<Value>(&visualization.setup).unwrap_or(Value::Null),
    })
}

/// `dashboard` as `viewer` reads it, its items from `visualizations`: its id, title, kind
/// (`saved`), whether it is the viewer's own, its shares (to its owner only) and its
/// Visualizations in order.
pub fn dashboard_json(
    dashboard: &SavedDashboard,
    visualizations: &[SavedVisualization],
    viewer: &Viewer,
) -> Value {
    let mine = dashboard.owner == viewer.account;
    let items: Vec<Value> = dashboard
        .items
        .iter()
        .filter_map(|id| visualizations.iter().find(|v| v.id == *id))
        .map(visualization_json)
        .collect();
    let mut out = json!({
        "id": dashboard.id,
        "title": dashboard.title,
        "kind": "saved",
        "mine": mine,
        "items": items,
    });
    if mine {
        out["shares"] = json!(dashboard.shares);
    }
    out
}

/// The viewer's own Visualizations on none of their Dashboards.
pub fn unplaced_json(
    dashboards: &[SavedDashboard],
    visualizations: &[SavedVisualization],
    viewer: &Viewer,
) -> Value {
    visualizations
        .iter()
        .filter(|v| v.owner == viewer.account)
        .filter(|v| !dashboards.iter().any(|d| d.items.contains(&v.id)))
        .map(visualization_json)
        .collect()
}

/// A standard Dashboard's id as viewers read it: its package's name and its own id, so one
/// from another package never clashes with it, and a new version keeps it.
pub fn standard_id(package: &str, dashboard: &str) -> String {
    format!("{package}:{dashboard}")
}

/// `item` as the explorer draws it on `today`: its periods resolved to a range, and no places,
/// since they follow the viewer. None when its periods do not read.
fn standard_item_json(item: &StandardItem, today: Date) -> Option<Value> {
    let shown = &item.visualization;
    let (from, to) = shown.question.range(today)?;
    let mut question = json!({
        "indicators": shown.question.indicators,
        "periodType": shown.question.period_type,
        "from": from,
        "to": to,
        "places": [],
    });
    if let Some(by) = &shown.question.disaggregation {
        question["disaggregation"] = json!(by);
    }
    Some(json!({
        "id": item.id,
        "title": item.title,
        "visualization": {
            "type": shown.kind,
            "question": question,
            "layout": shown.layout,
            "series": shown.series,
            "follow": shown.follow,
        },
    }))
}

/// `dashboard`, from version `version` of package `package`, as every viewer reads it on
/// `today`: kind `standard`, never anyone's own, and the package version it came from.
pub fn standard_json(
    package: &str,
    version: &str,
    dashboard: &StandardDashboard,
    today: Date,
) -> Value {
    let items: Vec<Value> = dashboard
        .items
        .iter()
        .filter_map(|item| standard_item_json(item, today))
        .collect();
    let mut out = json!({
        "id": standard_id(package, &dashboard.id),
        "title": dashboard.title,
        "kind": "standard",
        "mine": false,
        "package": format!("{package}@{version}"),
        "items": items,
    });
    if let Some(description) = &dashboard.description {
        out["description"] = json!(description);
    }
    out
}

/// The setup *Save as* keeps for a standard Dashboard's `item` (from [`standard_json`]), for
/// [`read_setup`] to check: as the author sees it, at `places`, still following its viewer.
pub fn copied_setup(item: &Value, places: &[String]) -> Value {
    let mut setup = item["visualization"].clone();
    setup["question"]["places"] = json!(places);
    setup
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use health_analytics::{Calculation, Counts};

    use super::*;

    fn visits() -> Indicator {
        Indicator {
            url: "https://x/Measure/anc1".into(),
            version: "1.0.0".into(),
            title: "ANC 1st visits".into(),
            scoring: Calculation::Count.scoring(),
            calculation: Calculation::Count,
            basis: "Encounter".into(),
            counts: Counts::People,
            cumulative: false,
            period_date: "Encounter.actualPeriod.start".into(),
            populations: BTreeMap::new(),
            references: BTreeSet::new(),
            disaggregations: Vec::new(),
            thresholds: Vec::new(),
            value: None,
        }
    }

    fn setup() -> Value {
        json!({
            "type": "column",
            "layout": "placesByIndicators",
            "series": "place",
            "follow": EACH_AREA_UNDER,
            "question": {
                "indicators": ["https://x/Measure/anc1|1.0.0"],
                "periodType": "monthly",
                "from": "2026-01",
                "to": "2026-03",
                "places": ["KAM013", "KAM020"],
                "unknown": true,
            },
            "extra": 1,
        })
    }

    #[test]
    fn a_setup_keeps_only_what_the_explorer_draws() {
        let read = read_setup(&setup(), &[visits()]).expect("setup");
        assert_eq!(read.places, ["KAM013", "KAM020"]);
        assert_eq!(read.follow.as_deref(), Some(EACH_AREA_UNDER));
        assert_eq!(read.json["extra"], Value::Null);
        assert_eq!(read.json["question"]["unknown"], Value::Null);
        assert_eq!(read.json["question"]["from"], "2026-01");
        assert_eq!(read.json["follow"], EACH_AREA_UNDER);
    }

    #[test]
    fn a_map_is_saved_like_any_other_type() {
        let mut body = setup();
        body["type"] = json!("map");
        let read = read_setup(&body, &[visits()]).expect("a map");
        assert_eq!(read.json["type"], "map");
    }

    #[test]
    fn a_setup_the_explorer_cannot_draw_is_refused() {
        for (path, value) in [
            ("type", json!("pie")),
            ("layout", json!(null)),
            ("series", json!("band")),
            ("follow", json!("everyone")),
        ] {
            let mut body = setup();
            body[path] = value;
            assert!(read_setup(&body, &[visits()]).is_err(), "{path}");
        }
        let mut dates = setup();
        dates["question"]["from"] = json!("2026-01-05");
        assert!(read_setup(&dates, &[visits()]).is_err());
        assert!(
            read_setup(&setup(), &[]).is_err(),
            "an indicator not offered"
        );
    }

    #[test]
    fn shares_name_people_roles_and_place_levels_once_each() {
        let body = json!({ "shares": [
            { "with": "level", "level": "district" },
            { "with": "role", "role": "district" },
            { "with": "person", "username": " e2e-wakiso " },
            { "with": "level", "level": "district" },
        ]});
        let levels = ["facility", "district"];
        let asked = read_shares(&body, "shares", &levels).expect("shares");
        assert_eq!(
            asked.shares,
            [
                Share::Level {
                    level: "district".into()
                },
                Share::Role {
                    role: "district".into()
                },
            ]
        );
        assert_eq!(asked.people, ["e2e-wakiso"]);
        for wrong in [
            json!({ "shares": [{ "with": "level", "level": "parish" }] }),
            json!({ "shares": [{ "with": "person" }] }),
            json!({ "shares": [{ "with": "everyone" }] }),
            json!({ "shares": "district" }),
        ] {
            assert!(read_shares(&wrong, "shares", &levels).is_err(), "{wrong}");
        }
        assert_eq!(
            read_shares(&json!({}), "shares", &levels).expect("none"),
            AskedShares::default()
        );
    }

    #[test]
    fn a_viewer_reads_the_items_in_order_and_only_the_owner_its_shares() {
        let saved = |id: &str, owner: &str| SavedVisualization {
            id: id.into(),
            owner: owner.into(),
            title: format!("V {id}"),
            setup: r#"{"type":"line"}"#.into(),
        };
        let visualizations = [
            saved("v1", "okot"),
            saved("v2", "okot"),
            saved("v3", "okot"),
        ];
        let dashboard = SavedDashboard {
            id: "d1".into(),
            owner: "okot".into(),
            title: "ANC".into(),
            shares: vec![Share::Level {
                level: "district".into(),
            }],
            items: vec!["v2".into(), "v1".into(), "gone".into()],
        };
        let viewer = |account: &str| Viewer {
            account: account.into(),
            roles: BTreeSet::new(),
            level: "district".into(),
        };
        let owner = dashboard_json(&dashboard, &visualizations, &viewer("okot"));
        assert_eq!(owner["mine"], true);
        assert_eq!(owner["items"][0]["id"], "v2");
        assert_eq!(owner["items"][1]["visualization"]["type"], "line");
        assert_eq!(owner["items"].as_array().map(Vec::len), Some(2));
        assert_eq!(owner["shares"][0]["level"], "district");
        let other = dashboard_json(&dashboard, &visualizations, &viewer("nakato"));
        assert_eq!(
            (other["mine"].clone(), other["shares"].clone()),
            (json!(false), Value::Null)
        );
        let unplaced = unplaced_json(&[dashboard], &visualizations, &viewer("okot"));
        assert_eq!(unplaced[0]["id"], "v3");
        assert_eq!(unplaced.as_array().map(Vec::len), Some(1));
    }

    fn standard() -> StandardDashboard {
        serde_json::from_value(json!({
            "id": "anc-overview",
            "title": "ANC overview",
            "description": "Antenatal care at a glance",
            "items": [{
                "id": "first-visits",
                "title": "ANC 1st visits by area",
                "visualization": {
                    "type": "column",
                    "question": {
                        "indicators": ["https://x/Measure/anc1|1.0.0"],
                        "periodType": "monthly",
                        "last": 3
                    },
                    "layout": "placesByIndicators",
                    "series": "place",
                    "follow": EACH_AREA_UNDER
                }
            }]
        }))
        .expect("a standard Dashboard")
    }

    #[test]
    fn a_standard_dashboard_reads_as_of_today_and_is_nobodys_own() {
        let read = standard_json(
            "ug.moh.hmis",
            "1.0.0",
            &standard(),
            Date::constant(2026, 11, 23),
        );
        assert_eq!(read["id"], "ug.moh.hmis:anc-overview");
        assert_eq!(
            (read["kind"].clone(), read["mine"].clone()),
            (json!("standard"), json!(false))
        );
        assert_eq!(read["package"], "ug.moh.hmis@1.0.0");
        let shown = &read["items"][0]["visualization"];
        assert_eq!(
            shown["question"],
            json!({
                "indicators": ["https://x/Measure/anc1|1.0.0"],
                "periodType": "monthly",
                "from": "2026-09",
                "to": "2026-11",
                "places": [],
            })
        );
        assert_eq!(shown["follow"], EACH_AREA_UNDER);
        // Save as keeps what the explorer saves, at the author's places.
        let copied = copied_setup(&read["items"][0], &["KAM013".to_owned()]);
        let setup = read_setup(&copied, &[visits()]).expect("a saved setup");
        assert_eq!(setup.places, ["KAM013"]);
        assert_eq!(setup.follow.as_deref(), Some(EACH_AREA_UNDER));
        assert_eq!(setup.json["question"]["from"], "2026-09");
    }
}
