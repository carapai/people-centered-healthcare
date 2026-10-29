//! Normalization of extracted values and search values into index form
//! (fhir-r5-model.md § Search parameters and indexed projections).

use jiff::civil::{Date, DateTime, Time};
use jiff::tz::Offset;
use jiff::{Span, Timestamp};
use rust_decimal::Decimal;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Case- and accent-folds a string: `Ñandú` becomes `nandu`.
pub fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// The resource type a reference points at: `Patient` for `Patient/1`,
/// `https://x/fhir/Patient/1` or `Patient/1/_history/2`.
pub fn reference_type(reference: &str) -> Option<&str> {
    split_reference(reference).map(|(t, _)| t)
}

fn split_reference(reference: &str) -> Option<(&str, &str)> {
    let without_history = reference
        .find("/_history/")
        .map_or(reference, |at| &reference[..at]);
    let mut segments = without_history.rsplit('/');
    let id = segments.next()?;
    let resource_type = segments.next()?;
    let valid_type = resource_type.starts_with(|c: char| c.is_ascii_uppercase())
        && resource_type.chars().all(|c| c.is_ascii_alphabetic());
    let valid_id = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    (valid_type && valid_id).then_some((resource_type, id))
}

/// A reference in index form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReferenceKey {
    /// The target type, or empty for a reference that names no type (a `urn:uuid:`, a canonical
    /// URL or a contained `#id`).
    pub target_type: String,
    /// The target id, or the whole reference when it names no type.
    pub target_id: String,
}

impl ReferenceKey {
    /// Normalizes a literal reference or canonical URL. Canonical versions (`url|1.0`) are
    /// dropped: a search by canonical URL matches every version.
    pub fn parse(reference: &str) -> Self {
        match split_reference(reference) {
            Some((target_type, target_id)) if !reference.contains('|') => Self {
                target_type: target_type.to_owned(),
                target_id: target_id.to_owned(),
            },
            _ => Self {
                target_type: String::new(),
                target_id: reference
                    .split_once('|')
                    .map_or(reference, |(url, _)| url)
                    .to_owned(),
            },
        }
    }
}

/// A half-open `[start, end)` instant range: a date's implicit range or a Period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateRange {
    /// Inclusive start.
    pub start: Timestamp,
    /// Exclusive end.
    pub end: Timestamp,
}

/// The earliest instant an open-start Period is taken to begin at.
pub fn date_floor() -> Timestamp {
    Timestamp::from_second(-62_135_596_800).unwrap_or(Timestamp::MIN) // 0001-01-01T00:00:00Z
}

/// The instant an open-ended Period is taken to end at.
pub fn date_ceiling() -> Timestamp {
    Timestamp::from_second(253_402_300_800).unwrap_or(Timestamp::MAX) // 10000-01-01T00:00:00Z
}

/// A date, dateTime or instant that cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a FHIR date, dateTime or instant")]
pub struct DateError(pub String);

/// The implicit range of a FHIR date, dateTime or instant at its stated precision: `2026` covers
/// the whole year and `2026-10-29T08:00:00.25Z` a hundredth of a second. A value without an
/// offset (every `date`, and a `dateTime` that omits one) is read at `local`, the node's
/// configured offset.
pub fn date_range(text: &str, local: Offset) -> Result<DateRange, DateError> {
    let error = || DateError(text.to_owned());
    let (date_part, time_part) = text
        .split_once('T')
        .map_or((text, None), |(d, t)| (d, Some(t)));
    let mut fields = date_part.split('-');
    let number = |s: Option<&str>, len: usize| -> Result<Option<i16>, DateError> {
        match s {
            None => Ok(None),
            Some(s) if s.len() == len && s.bytes().all(|b| b.is_ascii_digit()) => {
                s.parse().map(Some).map_err(|_| error())
            }
            Some(_) => Err(error()),
        }
    };
    let year = number(fields.next(), 4)?.ok_or_else(error)?;
    let month = number(fields.next(), 2)?;
    let day = number(fields.next(), 2)?;
    if fields.next().is_some() || (time_part.is_some() && day.is_none()) {
        return Err(error());
    }
    let month_i8 = |m: i16| i8::try_from(m).map_err(|_| error());
    let date = Date::new(
        year,
        month_i8(month.unwrap_or(1))?,
        month_i8(day.unwrap_or(1))?,
    )
    .map_err(|_| error())?;
    let Some(time_part) = time_part else {
        let step = match (month, day) {
            (None, _) => Span::new().years(1),
            (Some(_), None) => Span::new().months(1),
            (Some(_), Some(_)) => Span::new().days(1),
        };
        let start = date.to_datetime(Time::midnight());
        let end = start.checked_add(step).map_err(|_| error())?;
        return Ok(DateRange {
            start: local.to_timestamp(start).map_err(|_| error())?,
            end: local.to_timestamp(end).map_err(|_| error())?,
        });
    };
    let (clock, offset) = split_offset(time_part).ok_or_else(error)?;
    let offset = offset.unwrap_or(local);
    let (hms, fraction) = clock
        .split_once('.')
        .map_or((clock, None), |(a, b)| (a, Some(b)));
    let mut parts = hms.split(':');
    let hour = number(parts.next(), 2)?.ok_or_else(error)?;
    let minute = number(parts.next(), 2)?.ok_or_else(error)?;
    let second = number(parts.next(), 2)?;
    if parts.next().is_some() || (fraction.is_some() && second.is_none()) {
        return Err(error());
    }
    let nanos = match fraction {
        Some(f) if !f.is_empty() && f.len() <= 9 && f.bytes().all(|b| b.is_ascii_digit()) => {
            let padded = format!("{f:0<9}");
            padded.parse::<i32>().map_err(|_| error())?
        }
        Some(_) => return Err(error()),
        None => 0,
    };
    let time = Time::new(
        month_i8(hour)?,
        month_i8(minute)?,
        month_i8(second.unwrap_or(0))?,
        nanos,
    )
    .map_err(|_| error())?;
    let step = match (second, fraction) {
        (None, _) => Span::new().minutes(1),
        (Some(_), None) => Span::new().seconds(1),
        (Some(_), Some(f)) => {
            let exponent = u32::try_from(9 - f.len()).map_err(|_| error())?;
            Span::new().nanoseconds(10_i64.pow(exponent))
        }
    };
    let start: DateTime = date.to_datetime(time);
    let start = offset.to_timestamp(start).map_err(|_| error())?;
    let end = start.checked_add(step).map_err(|_| error())?;
    Ok(DateRange { start, end })
}

fn split_offset(time: &str) -> Option<(&str, Option<Offset>)> {
    if let Some(clock) = time.strip_suffix('Z') {
        return Some((clock, Some(Offset::UTC)));
    }
    let Some(at) = time.rfind(['+', '-']) else {
        return Some((time, None));
    };
    let (clock, zone) = time.split_at(at);
    let sign: i32 = if zone.starts_with('-') { -1 } else { 1 };
    let (hours, minutes) = zone[1..].split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let seconds = sign * (hours.parse::<i32>().ok()? * 3600 + minutes.parse::<i32>().ok()? * 60);
    Some((clock, Some(Offset::from_seconds(seconds).ok()?)))
}

/// A decimal that cannot be held exactly: more than 28 significant digits fails closed
/// (ADR 0052) rather than being rounded into the index.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a decimal of at most 28 significant digits")]
pub struct DecimalError(pub String);

/// Reads a JSON number's text exactly, keeping its scale (`1.50` stays `1.50`).
pub fn decimal(text: &str) -> Result<Decimal, DecimalError> {
    let parsed = if text.contains(['e', 'E']) {
        Decimal::from_scientific(text)
    } else {
        Decimal::from_str_exact(text)
    };
    parsed.map_err(|_| DecimalError(text.to_owned()))
}

/// The implicit range of a search number at its precision: `1.5` covers `[1.45, 1.55)` and
/// `100` covers `[99.5, 100.5)`.
pub fn decimal_range(value: Decimal) -> (Decimal, Decimal) {
    let half = Decimal::new(5, value.scale() + 1);
    (value - half, value + half)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(text: &str) -> (String, String) {
        let r = date_range(text, Offset::from_hours(3).expect("offset")).expect(text);
        (r.start.to_string(), r.end.to_string())
    }

    #[test]
    fn date_ranges_follow_precision_and_the_local_offset() {
        assert_eq!(
            range("2026"),
            ("2025-12-31T21:00:00Z".into(), "2026-12-31T21:00:00Z".into())
        );
        assert_eq!(
            range("2026-02"),
            ("2026-01-31T21:00:00Z".into(), "2026-02-28T21:00:00Z".into())
        );
        assert_eq!(
            range("2026-10-29"),
            ("2026-10-28T21:00:00Z".into(), "2026-10-29T21:00:00Z".into())
        );
        assert_eq!(
            range("2026-10-29T08:00Z"),
            ("2026-10-29T08:00:00Z".into(), "2026-10-29T08:01:00Z".into())
        );
        assert_eq!(
            range("2026-10-29T08:00:00.25+01:00"),
            (
                "2026-10-29T07:00:00.25Z".into(),
                "2026-10-29T07:00:00.26Z".into()
            )
        );
        for bad in [
            "26",
            "2026-13",
            "2026-10-29T",
            "2026-10-29T08",
            "2026-10-29T25:00Z",
        ] {
            assert!(date_range(bad, Offset::UTC).is_err(), "{bad}");
        }
    }

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Ñandú MÜLLER"), "nandu muller");
    }

    #[test]
    fn references_normalize_to_type_and_id() {
        let key = |t: &str, i: &str| ReferenceKey {
            target_type: t.into(),
            target_id: i.into(),
        };
        assert_eq!(ReferenceKey::parse("Patient/1"), key("Patient", "1"));
        assert_eq!(
            ReferenceKey::parse("https://x.ug/fhir/Patient/1/_history/3"),
            key("Patient", "1")
        );
        assert_eq!(
            ReferenceKey::parse("urn:uuid:0d7a"),
            key("", "urn:uuid:0d7a")
        );
        assert_eq!(
            ReferenceKey::parse("http://hl7.org/fhir/ValueSet/x|5.0.0"),
            key("", "http://hl7.org/fhir/ValueSet/x")
        );
    }

    #[test]
    fn decimals_keep_scale_and_fail_closed() {
        assert_eq!(decimal("1.50").expect("decimal").to_string(), "1.50");
        assert_eq!(decimal("1e2").expect("decimal").to_string(), "100");
        assert!(decimal("1.00000000000000000000000000001").is_err());
        let (low, high) = decimal_range(decimal("1.5").expect("decimal"));
        assert_eq!(
            (low.to_string(), high.to_string()),
            ("1.45".into(), "1.55".into())
        );
    }
}
