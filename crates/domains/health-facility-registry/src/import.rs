use std::collections::BTreeSet;

use serde::Serialize;

use crate::areas::AdministrativeAreas;
use crate::nhfr::NhfrFacility;
use crate::place::{Place, PlaceLevel, Places};

/// A place the import changes: as the registry holds it and as it will be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// As the registry holds it.
    pub before: Place,
    /// As the import leaves it.
    pub after: Place,
}

/// What an NHFR import would do, shown before it is applied (ADR 0094).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportPlan {
    /// The national place, regions and districts from the national core package that the
    /// registry does not hold yet, or holds differently.
    pub hierarchy: Vec<Place>,
    /// Facilities the registry does not hold yet (a closed one is added inactive).
    pub new: Vec<Place>,
    /// Facilities whose name, district, level, ownership or coordinates change, or that reopen.
    pub changed: Vec<Change>,
    /// Open facilities the NHFR now lists as closed: they become inactive, and are kept.
    pub closed: Vec<Change>,
    /// Facilities the import leaves as they are.
    pub unchanged: usize,
    /// Facilities in the registry the file does not list: left as they are, and shown so a
    /// partial export is noticed.
    pub absent: Vec<String>,
}

impl ImportPlan {
    /// Every place the import writes, the hierarchy first so each place's parent exists.
    pub fn writes(&self) -> Vec<&Place> {
        self.hierarchy
            .iter()
            .chain(&self.new)
            .chain(self.changed.iter().map(|c| &c.after))
            .chain(self.closed.iter().map(|c| &c.after))
            .collect()
    }
}

/// Why an import was refused: nothing is written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[error("the NHFR import was refused: {}", problems.join("; "))]
pub struct ImportRefused {
    /// Every problem found.
    pub problems: Vec<String>,
}

/// What importing `facilities` into the registry holding `current` would do, with the districts
/// and regions of `areas` (the national core package's). A facility whose district `areas` does
/// not hold refuses the import, as does a code used twice or one that is an area's.
pub fn plan_import(
    current: &Places,
    areas: &AdministrativeAreas,
    facilities: &[NhfrFacility],
) -> Result<ImportPlan, ImportRefused> {
    let mut problems = areas.problems();
    if areas.districts.is_empty() {
        problems.push("the national core package holds no districts".to_owned());
    }
    let hierarchy = areas.places();
    let area_codes: BTreeSet<&str> = hierarchy.iter().map(|p| p.code.as_str()).collect();
    let mut seen = BTreeSet::new();
    for facility in facilities {
        if !seen.insert(facility.code.as_str()) {
            problems.push(format!("{} is listed twice", facility.code));
        }
        if area_codes.contains(facility.code.as_str())
            || current
                .get(&facility.code)
                .is_some_and(|p| p.level != PlaceLevel::Facility)
        {
            problems.push(format!(
                "{} is a district's, region's or the national place's code",
                facility.code
            ));
        }
        if areas.district(&facility.district).is_none() {
            problems.push(format!(
                "{}'s district {:?} is not in the national core package",
                facility.code, facility.district
            ));
        }
    }
    if !problems.is_empty() {
        return Err(ImportRefused { problems });
    }
    let mut plan = ImportPlan {
        hierarchy: hierarchy
            .into_iter()
            .filter(|place| current.get(&place.code) != Some(place))
            .collect(),
        ..ImportPlan::default()
    };
    for facility in facilities {
        let after = Place {
            code: facility.code.clone(),
            name: facility.name.clone(),
            level: PlaceLevel::Facility,
            parent: Some(facility.district.clone()),
            active: facility.active,
            care_level: facility.care_level.clone(),
            ownership: facility.ownership.clone(),
            coordinates: facility.coordinates.clone(),
        };
        match current.get(&facility.code) {
            None => plan.new.push(after),
            Some(before) if *before == after => plan.unchanged += 1,
            Some(before) => {
                let change = Change {
                    before: before.clone(),
                    after,
                };
                if change.before.active && !change.after.active {
                    plan.closed.push(change);
                } else {
                    plan.changed.push(change);
                }
            }
        }
    }
    plan.absent = current
        .iter()
        .filter(|p| p.level == PlaceLevel::Facility && !seen.contains(p.code.as_str()))
        .map(|p| p.code.clone())
        .collect();
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Area, District};
    use crate::nhfr::parse_nhfr_csv;
    use crate::place::NATIONAL;

    fn areas() -> AdministrativeAreas {
        AdministrativeAreas {
            national: Some("Ministry of Health".into()),
            regions: vec![Area {
                code: "region-central".into(),
                name: "Central".into(),
            }],
            districts: vec![
                District {
                    code: "district-kampala".into(),
                    name: "Kampala".into(),
                    region: "region-central".into(),
                },
                District {
                    code: "district-wakiso".into(),
                    name: "Wakiso".into(),
                    region: "region-central".into(),
                },
            ],
        }
    }

    const FIRST: &str = "nhfr_id,name,district,status\n\
        KAM013,Kisenyi HC IV,district-kampala,open\n\
        KAM020,Kawempe HC III,district-kampala,open\n\
        WAK001,Wakiso HC IV,district-wakiso,open\n";

    #[test]
    fn a_first_import_creates_the_hierarchy_and_a_second_closes_a_facility() {
        let rows = parse_nhfr_csv(FIRST).expect("rows");
        let plan = plan_import(&Places::default(), &areas(), &rows).expect("plan");
        assert_eq!(plan.hierarchy.len(), 4);
        assert_eq!(plan.new.len(), 3);
        assert!(plan.changed.is_empty() && plan.closed.is_empty());
        assert_eq!(plan.writes()[0].code, NATIONAL);

        let registry = Places::new(plan.writes().into_iter().cloned());
        assert_eq!(
            registry.facilities_in("district-kampala"),
            ["KAM013", "KAM020"]
        );
        // The same file again changes nothing.
        let again = plan_import(&registry, &areas(), &rows).expect("plan");
        assert_eq!(again.unchanged, 3);
        assert!(again.writes().is_empty());

        let second = parse_nhfr_csv(
            "nhfr_id,name,district,status\n\
             KAM013,Kisenyi HC IV,district-kampala,open\n\
             KAM020,Kawempe HC III,district-kampala,closed\n\
             WAK001,Wakiso Health Centre IV,district-wakiso,open\n",
        )
        .expect("rows");
        let plan = plan_import(&registry, &areas(), &second).expect("plan");
        assert_eq!(plan.closed.len(), 1);
        assert_eq!(plan.closed[0].after.code, "KAM020");
        assert!(!plan.closed[0].after.active);
        assert_eq!(plan.changed.len(), 1);
        assert_eq!(plan.changed[0].after.name, "Wakiso Health Centre IV");
        assert_eq!(plan.unchanged, 1);
        let after = Places::new(
            registry
                .iter()
                .cloned()
                .chain(plan.writes().into_iter().cloned()),
        );
        // Closed, it stays in its district's area.
        assert_eq!(
            after.facilities_in("district-kampala"),
            ["KAM013", "KAM020"]
        );
        assert_eq!(after.get("KAM020").map(|p| p.active), Some(false));

        // A file that leaves a facility out lists it as absent and leaves it alone.
        let partial = parse_nhfr_csv(
            "nhfr_id,name,district,status\nKAM013,Kisenyi HC IV,district-kampala,open\n",
        )
        .expect("rows");
        let plan = plan_import(&after, &areas(), &partial).expect("plan");
        assert_eq!(plan.absent, ["KAM020", "WAK001"]);
        assert!(plan.writes().is_empty());
    }

    #[test]
    fn an_unknown_district_or_a_repeated_code_refuses_the_import() {
        let rows = parse_nhfr_csv(
            "nhfr_id,name,district,status\n\
             KAM013,Kisenyi HC IV,district-nowhere,open\n\
             WAK001,Wakiso HC IV,district-wakiso,open\n\
             WAK001,Wakiso HC IV,district-wakiso,open\n\
             district-wakiso,Odd,district-wakiso,open\n",
        )
        .expect("rows");
        let refused = plan_import(&Places::default(), &areas(), &rows).expect_err("refused");
        assert_eq!(refused.problems.len(), 3, "{:?}", refused.problems);
        assert!(refused.problems[0].contains("district-nowhere"));
        let empty = plan_import(&Places::default(), &AdministrativeAreas::default(), &[]);
        assert!(empty.is_err());
    }
}
