use crate::place::Coordinates;
use crate::reporting::is_dhis2_uid;

/// One facility as the NHFR's facility list gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NhfrFacility {
    /// Its NHFR identifier: the facility's code on the platform.
    pub code: String,
    /// Its name.
    pub name: String,
    /// Its district's code.
    pub district: String,
    /// Its level of care, if given.
    pub care_level: Option<String>,
    /// Its ownership, if given.
    pub ownership: Option<String>,
    /// False when the NHFR lists it as closed.
    pub active: bool,
    /// Where it is, if given.
    pub coordinates: Option<Coordinates>,
    /// Its DHIS2 org unit ID, if given (#560).
    pub org_unit: Option<String>,
}

/// The columns an NHFR export must have.
const REQUIRED: [&str; 4] = ["nhfr_id", "name", "district", "status"];

/// One CSV record's fields: commas separate, double quotes enclose a field that holds commas,
/// quotes (doubled) or line breaks.
fn records(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if field.is_empty() => quoted = true,
            (',', false) => record.push(std::mem::take(&mut field)),
            ('\n', false) => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            ('\r', false) => {}
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
        .into_iter()
        .filter(|r| r.iter().any(|f| !f.trim().is_empty()))
        .collect()
}

/// Reads an NHFR facility export (CSV, a header row first): the columns `nhfr_id`, `name`,
/// `district` and `status` (`open`, `functional` or `active`; `closed`, `non-functional` or
/// `inactive`), and optionally `level`, `ownership`, `latitude`, `longitude` and `dhis2_uid`
/// (the facility's DHIS2 org unit ID, #560), in any order.
/// Every problem is returned, each naming its line.
pub fn parse_nhfr_csv(text: &str) -> Result<Vec<NhfrFacility>, Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = records(text).into_iter();
    let Some(header) = rows.next() else {
        return Err(vec!["the file is empty".to_owned()]);
    };
    let header: Vec<String> = header
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let column = |name: &str| header.iter().position(|h| h == name);
    let missing: Vec<String> = REQUIRED
        .iter()
        .filter(|name| column(name).is_none())
        .map(|name| format!("the file has no {name} column"))
        .collect();
    if !missing.is_empty() {
        return Err(missing);
    }
    let mut facilities = Vec::new();
    let mut problems = Vec::new();
    for (index, row) in rows.enumerate() {
        let line = index + 2;
        let get = |name: &str| {
            column(name)
                .and_then(|i| row.get(i))
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        let (Some(code), Some(name), Some(district)) =
            (get("nhfr_id"), get("name"), get("district"))
        else {
            problems.push(format!(
                "line {line}: nhfr_id, name and district are required"
            ));
            continue;
        };
        let active = match get("status").map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("open" | "functional" | "active") => true,
            Some("closed" | "non-functional" | "inactive") => false,
            other => {
                problems.push(format!(
                    "line {line}: {code}'s status {:?} is not open or closed",
                    other.unwrap_or_default()
                ));
                continue;
            }
        };
        let coordinates = match (get("latitude"), get("longitude")) {
            (None, None) => None,
            (Some(latitude), Some(longitude)) => {
                let Some(position) = Coordinates::new(&latitude, &longitude) else {
                    problems.push(format!(
                        "line {line}: {code}'s coordinates are not decimal degrees"
                    ));
                    continue;
                };
                Some(position)
            }
            _ => {
                problems.push(format!(
                    "line {line}: {code} has only one of latitude and longitude"
                ));
                continue;
            }
        };
        let org_unit = get("dhis2_uid");
        if let Some(uid) = org_unit.as_deref().filter(|uid| !is_dhis2_uid(uid)) {
            problems.push(format!(
                "line {line}: {code}'s dhis2_uid {uid:?} is not a DHIS2 ID (11 letters and digits, a letter first)"
            ));
            continue;
        }
        facilities.push(NhfrFacility {
            code,
            name,
            district,
            care_level: get("level"),
            ownership: get("ownership"),
            active,
            coordinates,
            org_unit,
        });
    }
    if problems.is_empty() {
        Ok(facilities)
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_export_is_read_by_its_header() {
        let text = "\u{feff}status,nhfr_id,name,district,level,latitude,longitude,dhis2_uid\r\n\
            open,KAM013,\"Kisenyi HC IV, Kampala\",district-kampala,HC IV,0.3136,32.5664,KisenyiH4a1\r\n\
            Closed,KAM020,\"The \"\"Old\"\" Clinic\",district-kampala,,,,\r\n\
            \r\n";
        let facilities = parse_nhfr_csv(text).expect("facilities");
        assert_eq!(facilities.len(), 2);
        assert_eq!(facilities[0].name, "Kisenyi HC IV, Kampala");
        assert_eq!(facilities[0].care_level.as_deref(), Some("HC IV"));
        assert_eq!(
            facilities[0]
                .coordinates
                .as_ref()
                .map(|c| c.latitude.as_str()),
            Some("0.3136")
        );
        assert!(facilities[0].active);
        assert_eq!(facilities[0].org_unit.as_deref(), Some("KisenyiH4a1"));
        assert_eq!(facilities[1].org_unit, None);
        assert_eq!(facilities[1].name, "The \"Old\" Clinic");
        assert!(!facilities[1].active);
        assert_eq!(facilities[1].coordinates, None);
    }

    #[test]
    fn every_problem_is_reported_by_line() {
        assert_eq!(
            parse_nhfr_csv("nhfr_id,name\nX,Y\n"),
            Err(vec![
                "the file has no district column".to_owned(),
                "the file has no status column".to_owned()
            ])
        );
        let problems = parse_nhfr_csv(
            "nhfr_id,name,district,status,latitude,longitude\n\
             A,,d,open,,\n\
             B,b,d,maybe,,\n\
             C,c,d,open,200,1\n\
             D,d,d,open,1,\n",
        )
        .expect_err("problems");
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems[0].starts_with("line 2"));
        assert!(problems[3].starts_with("line 5"));
        assert!(parse_nhfr_csv("").is_err());
        let problems =
            parse_nhfr_csv("nhfr_id,name,district,status,dhis2_uid\nE,e,d,open,not-a-uid\n")
                .expect_err("problems");
        assert!(problems[0].contains("not a DHIS2 ID"), "{problems:?}");
    }
}
