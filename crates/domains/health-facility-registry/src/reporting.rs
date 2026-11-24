//! Each facility's HMIS reporting settings (ADR 0069; facility-configuration.md; #560), which
//! only central sets:
//!
//! - its **HMIS org unit ID**, the DHIS2 organisation unit central sends the facility's forms
//!   to, checked against the NHFR: [`org_unit_flags`] flags an ID the NHFR does not list, one it
//!   lists for another facility, one set for two facilities, and a facility the NHFR gives
//!   another ID. A flagged ID is kept (the NHFR may lag), and shown until it is put right;
//! - the **forms reported through the platform**: the DHIS2 data sets (HMIS 105, 108, 106a …)
//!   whose values central sends for the facility, so the district stops keying them. A form is
//!   reported through the platform only once the facility has an org unit ID.

use std::collections::BTreeSet;
use std::future::Future;

use health_persistence::{StoreError, WriteError};
use serde::{Deserialize, Serialize};

use crate::place::{PlaceLevel, Places};

/// Whether `text` is a DHIS2 ID (a UID): eleven ASCII letters and digits, a letter first.
pub fn is_dhis2_uid(text: &str) -> bool {
    text.len() == 11
        && text.chars().all(|c| c.is_ascii_alphanumeric())
        && text.starts_with(|c: char| c.is_ascii_alphabetic())
}

/// One facility's HMIS reporting settings, as central keeps them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilityReporting {
    /// The facility's code (its NHFR identifier).
    pub facility: String,
    /// Its HMIS org unit ID, once set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_unit: Option<String>,
    /// The IDs of the DHIS2 data sets reported through the platform for it.
    #[serde(default)]
    pub forms: BTreeSet<String>,
}

impl FacilityReporting {
    /// Why central refuses these settings, given the registry's `places` and the IDs of the
    /// data sets in its recorded copy of the DHIS2 instance's metadata: a place that is not a
    /// facility in the registry, an org unit ID that is not a DHIS2 ID, a form that is not one
    /// of those data sets, or forms reported through the platform without an org unit ID.
    pub fn problems(&self, places: &Places, data_sets: &BTreeSet<String>) -> Vec<String> {
        let mut problems = Vec::new();
        if places
            .get(&self.facility)
            .is_none_or(|p| p.level != PlaceLevel::Facility)
        {
            problems.push(format!(
                "{} is not a facility in the registry",
                self.facility
            ));
        }
        if let Some(uid) = self.org_unit.as_deref().filter(|uid| !is_dhis2_uid(uid)) {
            problems.push(format!(
                "the org unit ID {uid:?} is not a DHIS2 ID (11 letters and digits, a letter first)"
            ));
        }
        for form in self.forms.iter().filter(|f| !data_sets.contains(*f)) {
            problems.push(if data_sets.is_empty() {
                format!(
                    "{form} cannot be checked: central holds no copy of the DHIS2 instance's metadata"
                )
            } else {
                format!("{form} is not a data set of the DHIS2 instance")
            });
        }
        if !self.forms.is_empty() && self.org_unit.is_none() {
            problems.push(
                "a form is reported through the platform only once the facility has an org unit ID"
                    .to_owned(),
            );
        }
        problems
    }
}

/// Something wrong with a facility's HMIS org unit ID, found against the NHFR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OrgUnitFlag {
    /// The NHFR lists no facility with this org unit ID.
    #[serde(rename_all = "camelCase")]
    UnknownToNhfr {
        /// The ID set.
        org_unit: String,
    },
    /// The NHFR lists this org unit ID for another facility.
    #[serde(rename_all = "camelCase")]
    NhfrListsElsewhere {
        /// The ID set.
        org_unit: String,
        /// The facility the NHFR lists it for.
        facility: String,
    },
    /// The NHFR lists another org unit ID for this facility.
    #[serde(rename_all = "camelCase")]
    NhfrListsAnother {
        /// The ID set.
        org_unit: String,
        /// The NHFR's.
        nhfr: String,
    },
    /// Another facility is set to the same org unit ID.
    #[serde(rename_all = "camelCase")]
    SharedWith {
        /// The ID set.
        org_unit: String,
        /// The other facility.
        facility: String,
    },
}

/// What is wrong with `setting`'s org unit ID, checked against the NHFR's (each facility's
/// [`crate::Place::org_unit`] in `places`) and the other facilities' settings in `all`. None
/// when no ID is set.
pub fn org_unit_flags(
    places: &Places,
    all: &[FacilityReporting],
    setting: &FacilityReporting,
) -> Vec<OrgUnitFlag> {
    let Some(org_unit) = setting.org_unit.as_deref() else {
        return Vec::new();
    };
    let mut flags = Vec::new();
    let listed_for: Vec<&str> = places
        .iter()
        .filter(|p| p.org_unit.as_deref() == Some(org_unit))
        .map(|p| p.code.as_str())
        .collect();
    if listed_for.is_empty() {
        flags.push(OrgUnitFlag::UnknownToNhfr {
            org_unit: org_unit.to_owned(),
        });
    }
    for facility in listed_for.iter().filter(|f| **f != setting.facility) {
        flags.push(OrgUnitFlag::NhfrListsElsewhere {
            org_unit: org_unit.to_owned(),
            facility: (*facility).to_owned(),
        });
    }
    if let Some(nhfr) = places
        .get(&setting.facility)
        .and_then(|p| p.org_unit.as_deref())
        .filter(|nhfr| *nhfr != org_unit)
    {
        flags.push(OrgUnitFlag::NhfrListsAnother {
            org_unit: org_unit.to_owned(),
            nhfr: nhfr.to_owned(),
        });
    }
    for other in all
        .iter()
        .filter(|s| s.facility != setting.facility && s.org_unit.as_deref() == Some(org_unit))
    {
        flags.push(OrgUnitFlag::SharedWith {
            org_unit: org_unit.to_owned(),
            facility: other.facility.clone(),
        });
    }
    flags
}

/// Central's facility HMIS reporting settings, over a transaction. Implemented by the
/// persistence adapter.
pub trait FacilityReportingStore {
    /// Keeps `setting`, set by `by`, replacing the facility's earlier settings.
    fn put_facility_reporting(
        &mut self,
        setting: &FacilityReporting,
        by: &str,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Every facility's settings, by facility code.
    fn facility_reporting(
        &mut self,
    ) -> impl Future<Output = Result<Vec<FacilityReporting>, StoreError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::Place;

    fn facility(code: &str, org_unit: Option<&str>) -> Place {
        Place {
            code: code.into(),
            name: code.into(),
            level: PlaceLevel::Facility,
            parent: Some("district-kampala".into()),
            active: true,
            care_level: None,
            ownership: None,
            coordinates: None,
            org_unit: org_unit.map(str::to_owned),
        }
    }

    fn places() -> Places {
        Places::new([
            facility("KAM013", Some("KisenyiH4a1")),
            facility("KAM020", Some("KawempeH3b2")),
            facility("KAM031", None),
        ])
    }

    fn setting(facility: &str, org_unit: Option<&str>) -> FacilityReporting {
        FacilityReporting {
            facility: facility.into(),
            org_unit: org_unit.map(str::to_owned),
            forms: BTreeSet::new(),
        }
    }

    #[test]
    fn dhis2_ids_are_eleven_letters_and_digits_a_letter_first() {
        assert!(is_dhis2_uid("KisenyiH4a1"));
        assert!(!is_dhis2_uid("1isenyiH4a1"));
        assert!(!is_dhis2_uid("KisenyiH4a"));
        assert!(!is_dhis2_uid("Kisenyi-4a1"));
    }

    #[test]
    fn a_facility_with_an_org_unit_id_unknown_to_the_nhfr_is_flagged() {
        let set = setting("KAM031", Some("Unknown0001"));
        assert_eq!(
            org_unit_flags(&places(), std::slice::from_ref(&set), &set),
            [OrgUnitFlag::UnknownToNhfr {
                org_unit: "Unknown0001".into()
            }]
        );
        // The NHFR's own ID for the facility is not flagged.
        let good = setting("KAM013", Some("KisenyiH4a1"));
        assert!(org_unit_flags(&places(), std::slice::from_ref(&good), &good).is_empty());
        // Nor is a facility with none set.
        let none = setting("KAM020", None);
        assert!(org_unit_flags(&places(), &[], &none).is_empty());
    }

    #[test]
    fn an_org_unit_id_the_nhfr_gives_another_facility_or_set_twice_is_flagged() {
        let all = [
            setting("KAM013", Some("KawempeH3b2")),
            setting("KAM031", Some("KawempeH3b2")),
        ];
        assert_eq!(
            org_unit_flags(&places(), &all, &all[0]),
            [
                OrgUnitFlag::NhfrListsElsewhere {
                    org_unit: "KawempeH3b2".into(),
                    facility: "KAM020".into()
                },
                OrgUnitFlag::NhfrListsAnother {
                    org_unit: "KawempeH3b2".into(),
                    nhfr: "KisenyiH4a1".into()
                },
                OrgUnitFlag::SharedWith {
                    org_unit: "KawempeH3b2".into(),
                    facility: "KAM031".into()
                },
            ]
        );
    }

    #[test]
    fn settings_are_refused_for_a_non_facility_a_bad_id_or_an_unknown_form() {
        let data_sets = BTreeSet::from(["Hmis105Smpl".to_owned()]);
        let mut set = setting("KAM013", Some("KisenyiH4a1"));
        set.forms.insert("Hmis105Smpl".into());
        assert!(set.problems(&places(), &data_sets).is_empty());

        let mut bad = setting("district-kampala", Some("not-an-id"));
        bad.forms.insert("Hmis999Smpl".into());
        let problems = bad.problems(&places(), &data_sets);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].contains("not a facility"));
        assert!(problems[1].contains("not a DHIS2 ID"));
        assert!(problems[2].contains("not a data set"));

        // Without the instance's metadata no form can be checked.
        assert!(set.problems(&places(), &BTreeSet::new())[0].contains("cannot be checked"));

        // A form needs an org unit ID to be sent to.
        let mut unsent = setting("KAM031", None);
        unsent.forms.insert("Hmis105Smpl".into());
        assert_eq!(
            unsent.problems(&places(), &data_sets),
            ["a form is reported through the platform only once the facility has an org unit ID"]
        );
    }
}
