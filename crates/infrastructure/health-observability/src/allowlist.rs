//! The telemetry attribute allowlist (ADR 0047; deployment.md § Observability).
//!
//! A field reaches an exported span or log only if its **name** is on the allowlist and its
//! **value** has the shape that name allows. Everything else is dropped and counted, so a
//! mistake such as `info!(patient = %name)` or `resource_id = %phone` exports nothing.
//!
//! - Allowed: resource UUIDs, node and facility ids, operation names, codes, durations and
//!   counts.
//! - Never allowed: names, phone numbers, NINs, free text. Event messages are free text, so they
//!   are never exported; an event is identified by its target, level and callsite.

/// What a field's value must look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A hyphenated UUID (resource, version, operation, node and device ids).
    Uuid,
    /// A short identifier: `[A-Za-z0-9._:-]{1,64}` with no run of seven or more digits (which
    /// would fit a phone number or an NIN), e.g. `KLA01`, `Observation`, `fhir.read`.
    Ident,
    /// An HTTP route template: `[A-Za-z0-9/_{}:.$*-]{1,256}`, never a concrete path.
    Route,
    /// A code from a non-sensitive ValueSet: `[A-Za-z0-9._:|/-]{1,128}`. Callers must not record
    /// codes from sensitive ValueSets (HIV, SRH, mental health).
    Code,
    /// A number or boolean; strings are refused even if they look numeric.
    Number,
}

/// The shape a field name allows, or `None` if the name is not on the allowlist.
pub fn shape(name: &str) -> Option<Shape> {
    Some(match name {
        "operation" | "outcome" | "error.kind" | "resource_type" | "http.method"
        | "facility_id" | "tenant" | "engine" | "check" => Shape::Ident,
        "http.route" => Shape::Route,
        "code" => Shape::Code,
        "http.status_code" | "count" | "attempt" => Shape::Number,
        _ if name.ends_with("_id") => Shape::Uuid,
        _ if [
            "_ms", "_count", "_bytes", "_seconds", "_total", "_depth", "_lag",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix)) =>
        {
            Shape::Number
        }
        _ => return None,
    })
}

fn chars_in(text: &str, max: usize, extra: &str) -> bool {
    !text.is_empty()
        && text.len() <= max
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn has_digit_run(text: &str, length: usize) -> bool {
    text.as_bytes()
        .split(|b| !b.is_ascii_digit())
        .any(|run| run.len() >= length)
}

/// Whether a string value fits `shape`. Numbers never arrive as strings.
pub fn admits(shape: Shape, text: &str) -> bool {
    match shape {
        Shape::Uuid => is_uuid(text),
        Shape::Ident => chars_in(text, 64, "._:-") && !has_digit_run(text, 7),
        Shape::Route => chars_in(text, 256, "/_{}:.$*-") && !has_digit_run(text, 7),
        Shape::Code => chars_in(text, 128, "._:|/-"),
        Shape::Number => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_choose_shapes_and_values_must_fit() {
        assert_eq!(shape("resource_id"), Some(Shape::Uuid));
        assert_eq!(shape("duration_ms"), Some(Shape::Number));
        assert_eq!(shape("facility_id"), Some(Shape::Ident));
        for name in [
            "name", "patient", "phone", "nin", "message", "note", "given", "query", "path",
        ] {
            assert_eq!(shape(name), None, "{name}");
        }
        assert!(admits(Shape::Uuid, "0192f1a2-7b3c-7d4e-8f90-1a2b3c4d5e6f"));
        assert!(!admits(Shape::Uuid, "CM900123456789"));
        assert!(admits(Shape::Ident, "fhir.read"));
        assert!(
            !admits(Shape::Ident, "Akello Grace"),
            "spaces are free text"
        );
        assert!(!admits(Shape::Ident, "0772123456"), "a phone number");
        assert!(!admits(Shape::Ident, "CM90012345678X"), "an NIN");
        assert!(admits(Shape::Route, "/fhir/{resource_type}/{id}"));
        assert!(!admits(Shape::Route, "/fhir/Patient?name=Akello"));
        assert!(!admits(Shape::Number, "42"));
    }
}
