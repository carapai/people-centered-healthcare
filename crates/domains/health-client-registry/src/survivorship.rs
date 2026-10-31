use health_time::Timestamp;

/// Where a value comes from, highest trust first (patient-identity.md § Survivorship).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Trust {
    /// Verified with NIRA.
    NiraVerified = 1,
    /// Verified by staff in person against an ID document.
    StaffVerified = 2,
    /// Entered by staff.
    StaffEntered = 3,
    /// Reported by the patient.
    SelfReported = 4,
}

impl Trust {
    /// The level, 1 (highest) to 4.
    pub fn level(self) -> u8 {
        self as u8
    }

    /// The level's trust, if valid.
    pub fn from_level(level: u8) -> Option<Self> {
        match level {
            1 => Some(Self::NiraVerified),
            2 => Some(Self::StaffVerified),
            3 => Some(Self::StaffEntered),
            4 => Some(Self::SelfReported),
            _ => None,
        }
    }
}

/// A golden-record field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    /// Family name.
    Family,
    /// Given names, space-separated.
    Given,
    /// Administrative gender.
    Gender,
    /// Birth date (FHIR date, possibly year only).
    BirthDate,
    /// National Identification Number.
    Nin,
    /// Mobile phone.
    Phone,
    /// District of residence.
    District,
}

impl Field {
    /// Every field, in order.
    pub const ALL: [Self; 7] = [
        Self::Family,
        Self::Given,
        Self::Gender,
        Self::BirthDate,
        Self::Nin,
        Self::Phone,
        Self::District,
    ];

    /// The field's code, as recorded in a value's origin.
    pub fn code(self) -> &'static str {
        match self {
            Self::Family => "family",
            Self::Given => "given",
            Self::Gender => "gender",
            Self::BirthDate => "birthDate",
            Self::Nin => "nin",
            Self::Phone => "phone",
            Self::District => "district",
        }
    }

    /// The field with `code`.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.code() == code)
    }
}

/// One value for a field, with its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The value.
    pub value: String,
    /// How far it is trusted.
    pub trust: Trust,
    /// When it was recorded.
    pub recorded: Timestamp,
    /// The source Patient it came from.
    pub source: String,
}

/// A value that cannot survive and needs a reviewer (§62): a lower-trust value that disagrees
/// with the golden record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewItem {
    /// The field.
    pub field: Field,
    /// The surviving value.
    pub current: Claim,
    /// The value that was not taken.
    pub incoming: Claim,
}

/// What survivorship decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Survival {
    /// The incoming value replaces (or becomes) the field's value.
    Adopted,
    /// The current value stays; the incoming one agrees or is older at the same trust.
    Kept,
    /// The current value stays and the disagreement goes to review.
    Review(ReviewItem),
}

/// Survivorship for one field: higher trust wins, and the most recent wins within a level. A
/// lower-trust value never overwrites a higher-trust one; if it disagrees, it is reviewed.
pub fn survive(field: Field, current: Option<&Claim>, incoming: &Claim) -> Survival {
    let Some(current) = current else {
        return Survival::Adopted;
    };
    if incoming.trust < current.trust {
        return Survival::Adopted;
    }
    if incoming.trust == current.trust {
        // The most recent value wins within a level (it also refreshes the origin).
        return if incoming.recorded > current.recorded {
            Survival::Adopted
        } else {
            Survival::Kept
        };
    }
    if incoming.value == current.value {
        Survival::Kept
    } else {
        Survival::Review(ReviewItem {
            field,
            current: current.clone(),
            incoming: incoming.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(value: &str, trust: Trust, at: i64, source: &str) -> Claim {
        Claim {
            value: value.into(),
            trust,
            recorded: Timestamp::from_second(at).unwrap_or(Timestamp::UNIX_EPOCH),
            source: source.into(),
        }
    }

    #[test]
    fn higher_trust_wins_and_recency_breaks_ties() {
        let staff = claim("Okello", Trust::StaffEntered, 100, "a");
        assert_eq!(survive(Field::Family, None, &staff), Survival::Adopted);
        assert_eq!(
            survive(
                Field::Family,
                Some(&staff),
                &claim("Okelo", Trust::NiraVerified, 50, "b")
            ),
            Survival::Adopted
        );
        assert_eq!(
            survive(
                Field::Family,
                Some(&staff),
                &claim("Okelo", Trust::StaffEntered, 200, "b")
            ),
            Survival::Adopted
        );
        assert_eq!(
            survive(
                Field::Family,
                Some(&staff),
                &claim("Okelo", Trust::StaffEntered, 50, "b")
            ),
            Survival::Kept
        );
    }

    #[test]
    fn a_lower_trust_value_never_overwrites_and_disagreement_is_reviewed() {
        let nira = claim("CM8601234567AB", Trust::NiraVerified, 100, "a");
        let later_self = claim("CM8601234567XX", Trust::SelfReported, 999, "b");
        let Survival::Review(item) = survive(Field::Nin, Some(&nira), &later_self) else {
            panic!("reviewed");
        };
        assert_eq!(
            (item.field, item.current, item.incoming),
            (Field::Nin, nira.clone(), later_self)
        );
        // Agreement at lower trust changes nothing.
        assert_eq!(
            survive(
                Field::Nin,
                Some(&nira),
                &claim("CM8601234567AB", Trust::SelfReported, 999, "b")
            ),
            Survival::Kept
        );
    }

    #[test]
    fn levels_and_codes_round_trip() {
        for level in 1..=4 {
            assert_eq!(Trust::from_level(level).map(Trust::level), Some(level));
        }
        assert_eq!(Trust::from_level(5), None);
        for field in Field::ALL {
            assert_eq!(Field::from_code(field.code()), Some(field));
        }
    }
}
