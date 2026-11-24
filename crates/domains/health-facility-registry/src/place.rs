use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The national place's code: ADR 0056's reserved facility code `central`, which ADR 0094 makes
/// the national Organization.
pub const NATIONAL: &str = "central";

/// The namespace of places' derived ids (UUIDv5; identifiers.md).
const PLACE_NAMESPACE: Uuid = Uuid::from_u128(0x0190_f1a2_7c3d_7e4f_8a5b_0000_0869_0001);

/// The Organization id of the place with `code`: the same on every node, so a place is read by
/// its code without a search.
pub fn place_id(code: &str) -> Uuid {
    Uuid::new_v5(&PLACE_NAMESPACE, format!("Organization/{code}").as_bytes())
}

/// The id of the Location holding a facility's coordinates.
pub fn location_id(code: &str) -> Uuid {
    Uuid::new_v5(&PLACE_NAMESPACE, format!("Location/{code}").as_bytes())
}

/// Where a place sits in the hierarchy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum PlaceLevel {
    /// A health facility, where patient-level records are kept.
    #[default]
    Facility,
    /// A district: its facilities.
    District,
    /// A region: its districts.
    Region,
    /// The country: its regions.
    National,
}

impl PlaceLevel {
    /// The level's code, as the token's `place-level` claim and an Organization's type carry it.
    pub fn code(self) -> &'static str {
        match self {
            Self::Facility => "facility",
            Self::District => "district",
            Self::Region => "region",
            Self::National => "national",
        }
    }

    /// Whether a session at a place of this level acts for an area rather than a facility: it
    /// reads summaries and Reports, never patient-level records (ADR 0094).
    pub fn is_area(self) -> bool {
        self != Self::Facility
    }

    /// The level directly above, if any.
    pub fn parent(self) -> Option<Self> {
        match self {
            Self::Facility => Some(Self::District),
            Self::District => Some(Self::Region),
            Self::Region => Some(Self::National),
            Self::National => None,
        }
    }
}

impl fmt::Display for PlaceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Not a place level.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a place level")]
pub struct UnknownLevel(pub String);

impl FromStr for PlaceLevel {
    type Err = UnknownLevel;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "facility" => Ok(Self::Facility),
            "district" => Ok(Self::District),
            "region" => Ok(Self::Region),
            "national" => Ok(Self::National),
            _ => Err(UnknownLevel(text.to_owned())),
        }
    }
}

/// A facility's position, as the NHFR gives it: decimal degrees, kept as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coordinates {
    /// −90 to 90.
    pub latitude: String,
    /// −180 to 180.
    pub longitude: String,
}

impl Coordinates {
    /// The coordinates, when both are decimal degrees in range.
    pub fn new(latitude: &str, longitude: &str) -> Option<Self> {
        let degrees = |text: &str, limit: f64| {
            let text = text.trim();
            let valid = !text.is_empty()
                && text
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
                && text.parse::<f64>().is_ok_and(|v| v.abs() <= limit);
            valid.then(|| text.to_owned())
        };
        Some(Self {
            latitude: degrees(latitude, 90.0)?,
            longitude: degrees(longitude, 180.0)?,
        })
    }
}

/// One place in the hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    /// Its code: a facility's NHFR identifier, a district's or region's code from the national
    /// core package, or [`NATIONAL`].
    pub code: String,
    /// Its name.
    pub name: String,
    /// Its level.
    pub level: PlaceLevel,
    /// The place directly above; none for the national place.
    pub parent: Option<String>,
    /// False once the NHFR closes it. A closed place is kept: records and summaries still name
    /// it.
    pub active: bool,
    /// The NHFR's level of care (`HC III`, `Hospital`, …), for a facility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub care_level: Option<String>,
    /// Its ownership (`Government`, `PNFP`, …), for a facility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<String>,
    /// Where it is, for a facility that has a position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    /// The facility's DHIS2 org unit ID as the NHFR lists it, if it does (#560): what the HMIS
    /// org unit ID central sets for it is checked against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_unit: Option<String>,
}

impl Place {
    /// The national place, named `name`.
    pub fn national(name: &str) -> Self {
        Self {
            code: NATIONAL.to_owned(),
            name: name.to_owned(),
            level: PlaceLevel::National,
            parent: None,
            active: true,
            care_level: None,
            ownership: None,
            coordinates: None,
            org_unit: None,
        }
    }
}

/// The registry's places, by code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Places {
    places: BTreeMap<String, Place>,
    children: BTreeMap<String, BTreeSet<String>>,
}

impl Places {
    /// The hierarchy of `places`; a later place with the same code replaces an earlier one.
    pub fn new(places: impl IntoIterator<Item = Place>) -> Self {
        let places: BTreeMap<String, Place> =
            places.into_iter().map(|p| (p.code.clone(), p)).collect();
        let mut children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for place in places.values() {
            if let Some(parent) = &place.parent {
                children
                    .entry(parent.clone())
                    .or_default()
                    .insert(place.code.clone());
            }
        }
        Self { places, children }
    }

    /// The place with `code`.
    pub fn get(&self, code: &str) -> Option<&Place> {
        self.places.get(code)
    }

    /// Every place, by code.
    pub fn iter(&self) -> impl Iterator<Item = &Place> {
        self.places.values()
    }

    /// How many places there are.
    pub fn len(&self) -> usize {
        self.places.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.places.is_empty()
    }

    /// The level of the place with `code`: [`NATIONAL`] is always the national place, and a code
    /// the registry does not hold is a facility, as every session was before ADR 0094.
    pub fn level_of(&self, code: &str) -> PlaceLevel {
        if code == NATIONAL {
            return PlaceLevel::National;
        }
        self.get(code).map_or(PlaceLevel::Facility, |p| p.level)
    }

    /// The area of `code`: the place and everything below it, the place first and the rest by
    /// code. Empty when the registry does not hold it.
    pub fn area(&self, code: &str) -> Vec<&Place> {
        let Some(top) = self.get(code) else {
            return Vec::new();
        };
        let mut seen = BTreeSet::from([code]);
        let mut pending = vec![code];
        let mut below = Vec::new();
        while let Some(next) = pending.pop() {
            for child in self.children.get(next).into_iter().flatten() {
                if seen.insert(child.as_str())
                    && let Some(place) = self.get(child)
                {
                    below.push(place);
                    pending.push(child.as_str());
                }
            }
        }
        below.sort_by(|a, b| a.code.cmp(&b.code));
        let mut area = vec![top];
        area.extend(below);
        area
    }

    /// The codes of the facilities in the area of `code`, sorted: what its figures roll up over.
    /// Closed facilities are included, since their summaries still count for the periods they
    /// were open.
    pub fn facilities_in(&self, code: &str) -> Vec<String> {
        let mut codes: Vec<String> = self
            .area(code)
            .into_iter()
            .filter(|p| p.level == PlaceLevel::Facility)
            .map(|p| p.code.clone())
            .collect();
        codes.sort();
        codes
    }

    /// Whether `code` is in the area of `place`.
    pub fn within(&self, code: &str, place: &str) -> bool {
        let mut at = Some(code);
        let mut steps = 0;
        while let Some(current) = at {
            if current == place {
                return true;
            }
            steps += 1;
            if steps > self.places.len() {
                return false;
            }
            at = self.get(current).and_then(|p| p.parent.as_deref());
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(code: &str, level: PlaceLevel, parent: Option<&str>) -> Place {
        Place {
            code: code.into(),
            name: code.into(),
            level,
            parent: parent.map(str::to_owned),
            active: true,
            care_level: None,
            ownership: None,
            coordinates: None,
            org_unit: None,
        }
    }

    fn sample() -> Places {
        Places::new([
            Place::national("Ministry of Health"),
            place("region-central", PlaceLevel::Region, Some(NATIONAL)),
            place("region-east", PlaceLevel::Region, Some(NATIONAL)),
            place(
                "district-kampala",
                PlaceLevel::District,
                Some("region-central"),
            ),
            place(
                "district-wakiso",
                PlaceLevel::District,
                Some("region-central"),
            ),
            place("district-jinja", PlaceLevel::District, Some("region-east")),
            place("KAM013", PlaceLevel::Facility, Some("district-kampala")),
            place("KAM020", PlaceLevel::Facility, Some("district-kampala")),
            place("WAK001", PlaceLevel::Facility, Some("district-wakiso")),
            place("JIN001", PlaceLevel::Facility, Some("district-jinja")),
        ])
    }

    #[test]
    fn a_district_s_area_is_it_and_its_facilities() {
        let places = sample();
        let area: Vec<&str> = places
            .area("district-kampala")
            .iter()
            .map(|p| p.code.as_str())
            .collect();
        assert_eq!(area, ["district-kampala", "KAM013", "KAM020"]);
        assert_eq!(
            places.facilities_in("district-kampala"),
            ["KAM013", "KAM020"]
        );
        assert_eq!(
            places.facilities_in("region-central"),
            ["KAM013", "KAM020", "WAK001"]
        );
        assert_eq!(places.facilities_in(NATIONAL).len(), 4);
        assert_eq!(places.facilities_in("KAM013"), ["KAM013"]);
        assert!(places.area("nowhere").is_empty());
        assert!(places.within("KAM013", "region-central"));
        assert!(!places.within("JIN001", "region-central"));
    }

    #[test]
    fn levels_come_from_the_registry_with_central_national() {
        let places = sample();
        assert_eq!(places.level_of("district-kampala"), PlaceLevel::District);
        assert_eq!(places.level_of(NATIONAL), PlaceLevel::National);
        assert_eq!(Places::default().level_of(NATIONAL), PlaceLevel::National);
        // A facility code the registry does not hold yet is a facility, as before.
        assert_eq!(places.level_of("UNLISTED"), PlaceLevel::Facility);
        assert!(!PlaceLevel::Facility.is_area());
        assert!(PlaceLevel::District.is_area());
        for level in [
            PlaceLevel::Facility,
            PlaceLevel::District,
            PlaceLevel::Region,
            PlaceLevel::National,
        ] {
            assert_eq!(level.code().parse::<PlaceLevel>(), Ok(level));
        }
    }

    #[test]
    fn a_cycle_does_not_loop() {
        let places = Places::new([
            place("a", PlaceLevel::District, Some("b")),
            place("b", PlaceLevel::District, Some("a")),
        ]);
        assert_eq!(places.area("a").len(), 2);
        assert!(!places.within("a", "c"));
    }

    #[test]
    fn coordinates_are_decimal_degrees_in_range() {
        assert!(Coordinates::new("0.3476", "32.5825").is_some());
        assert!(Coordinates::new("-1.2", "180").is_some());
        assert!(Coordinates::new("91", "32").is_none());
        assert!(Coordinates::new("1e2", "32").is_none());
        assert!(Coordinates::new("", "32").is_none());
        assert_ne!(place_id("KAM013"), location_id("KAM013"));
        assert_eq!(place_id("KAM013"), place_id("KAM013"));
    }
}
