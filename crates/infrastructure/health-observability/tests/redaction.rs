//! The redaction test (ADR 0047): names, phone numbers, NINs and free text never reach exported
//! spans or logs, however they are recorded.

use std::sync::{Arc, Mutex};

use health_observability::{ExportLayer, LogSink};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use tracing_subscriber::layer::SubscriberExt;

const NAME: &str = "Akello Grace";
const PHONE: &str = "+256772123456";
const LOCAL_PHONE: &str = "0772123456";
const NIN: &str = "CM90012345678X";
const NOTE: &str = "complains of chest pain since Tuesday";
const RESOURCE: &str = "0192f1a2-7b3c-7d4e-8f90-1a2b3c4d5e6f";

#[derive(Default)]
struct Lines(Mutex<Vec<String>>);

impl LogSink for Lines {
    fn write(&self, line: &str) {
        self.0.lock().expect("lines").push(line.to_owned());
    }
}

#[derive(Debug)]
#[allow(dead_code)] // read through Debug only
struct PatientSummary {
    name: &'static str,
    phone: &'static str,
}

/// Records sensitive values every way the `tracing` API allows, in spans, span records and
/// events, under allowlisted and other names.
fn emit() {
    let io = std::io::Error::other(format!("lookup failed for {NAME} {PHONE}"));
    let summary = PatientSummary {
        name: NAME,
        phone: PHONE,
    };
    let request = tracing::info_span!(
        "fhir.read",
        resource_id = RESOURCE,
        resource_type = "Patient",
        operation = "fhir.read",
        patient_name = NAME,
        phone = PHONE,
        nin = NIN,
        note = NOTE,
        outcome = tracing::field::Empty,
    );
    let _entered = request.enter();
    {
        let inner = tracing::info_span!(
            "registry.lookup",
            // Allowlisted names carrying the wrong kind of value.
            resource_id = PHONE,
            facility_id = LOCAL_PHONE,
            operation = %NAME,
            tenant = NIN,
            code = %NOTE,
            duration_ms = "12",
            http.route = "/fhir/Patient?name=Akello",
            summary = ?summary,
        );
        let _inner = inner.enter();
        tracing::info!(
            count = 3,
            duration_ms = 4.5,
            name = NAME,
            phone = PHONE,
            "registered {NAME} {PHONE} {NIN}: {NOTE}"
        );
        tracing::warn!(
            error = &io as &(dyn std::error::Error + 'static),
            "lookup failed"
        );
        tracing::error!(resource_id = %LOCAL_PHONE, check = ?NAME, "{summary:?}");
    }
    request.record("outcome", NAME);
    tracing::info!(outcome = "found", http.status_code = 200_u64, "done");
}

#[test]
fn sensitive_values_never_reach_exported_spans_or_logs() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let lines = Arc::new(Lines::default());
    let subscriber = tracing_subscriber::registry().with(ExportLayer::new(
        Some(provider.tracer("redaction-test")),
        lines.clone(),
    ));
    tracing::subscriber::with_default(subscriber, emit);
    provider.force_flush().expect("flush");

    let spans = exporter.get_finished_spans().expect("spans");
    let logs = lines.0.lock().expect("lines").clone();
    assert_eq!(spans.len(), 2, "both spans exported");
    assert_eq!(logs.len(), 4, "every event logged");

    let exported = format!("{spans:?}\n{}", logs.join("\n"));
    for secret in [
        NAME,
        "Akello",
        "Grace",
        PHONE,
        "256772123456",
        LOCAL_PHONE,
        NIN,
        NOTE,
        "chest pain",
        "Tuesday",
        "?name=",
    ] {
        assert!(!exported.contains(secret), "`{secret}` was exported");
    }

    // What is allowed still gets through, and what was dropped is counted.
    let outer = spans
        .iter()
        .find(|s| s.name == "fhir.read")
        .expect("outer span");
    let inner = spans
        .iter()
        .find(|s| s.name == "registry.lookup")
        .expect("inner span");
    assert_eq!(inner.parent_span_id, outer.span_context.span_id());
    let attribute = |span: &opentelemetry_sdk::trace::SpanData, key: &str| {
        span.attributes
            .iter()
            .find(|kv| kv.key.as_str() == key)
            .map(|kv| kv.value.to_string())
    };
    assert_eq!(attribute(outer, "resource_id").as_deref(), Some(RESOURCE));
    assert_eq!(attribute(outer, "operation").as_deref(), Some("fhir.read"));
    assert_eq!(attribute(outer, "outcome"), None);
    assert_eq!(
        attribute(outer, "telemetry.redacted_fields").as_deref(),
        Some("5")
    );
    assert_eq!(
        attribute(inner, "telemetry.redacted_fields").as_deref(),
        Some("8")
    );
    assert!(logs[0].contains(r#""count":3"#) && logs[0].contains(r#""duration_ms":4.5"#));
    assert!(
        logs[0].contains(r#""telemetry.redacted_fields":2"#),
        "{}",
        logs[0]
    );
    assert!(
        logs[3].contains(r#""http.status_code":200"#) && logs[3].contains(r#""outcome":"found""#)
    );
    assert!(
        logs.iter().all(|l| l.contains(r#""trace_id""#)),
        "logs correlate with traces"
    );
    assert!(
        logs[3].contains(&format!(r#""span_fields":{{"resource_id":"{RESOURCE}""#)),
        "log lines repeat the span's allowed fields: {}",
        logs[3]
    );
}
