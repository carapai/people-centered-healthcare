//! The export layer: the only path from `tracing` to exported spans and logs.
//!
//! It reads every span and event through the [allowlist](crate::allowlist) and builds the
//! OpenTelemetry spans and JSON log lines itself, so no other layer's view of the fields can be
//! exported by mistake. Dropped fields are counted in `telemetry.redacted_fields`.

use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;

use opentelemetry::trace::{SpanBuilder, TraceContextExt, Tracer};
use opentelemetry::{Context, KeyValue};
use opentelemetry_sdk::trace::SdkTracer;
use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context as LayerContext;
use tracing_subscriber::registry::LookupSpan;

use crate::allowlist::{Shape, admits, shape};

/// Where exported log lines go: one JSON object per line.
pub trait LogSink: Send + Sync + 'static {
    /// Writes one line, without the newline.
    fn write(&self, line: &str);
}

/// Log lines to standard output, collected by the node's OpenTelemetry collector.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stdout;

impl LogSink for Stdout {
    fn write(&self, line: &str) {
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
    }
}

#[derive(Debug, Clone)]
enum Attr {
    Str(String),
    I64(i64),
    F64(f64),
    Bool(bool),
}

/// The fields of one span or event that passed the allowlist.
#[derive(Debug, Default)]
struct Fields {
    kept: Vec<(&'static str, Attr)>,
    redacted: u32,
}

impl Fields {
    fn string(&mut self, field: &Field, text: &str) {
        match shape(field.name()) {
            Some(s) if s != Shape::Number && admits(s, text) => {
                self.kept.push((field.name(), Attr::Str(text.to_owned())));
            }
            _ => self.redacted += 1,
        }
    }

    fn number(&mut self, field: &Field, value: Attr) {
        if shape(field.name()) == Some(Shape::Number) {
            self.kept.push((field.name(), value));
        } else {
            self.redacted += 1;
        }
    }

    fn key_values(&self) -> Vec<KeyValue> {
        self.kept
            .iter()
            .map(|(name, value)| match value {
                Attr::Str(s) => KeyValue::new(*name, s.clone()),
                Attr::I64(n) => KeyValue::new(*name, *n),
                Attr::F64(n) => KeyValue::new(*name, *n),
                Attr::Bool(b) => KeyValue::new(*name, *b),
            })
            .collect()
    }

    fn json(&self) -> Map<String, Value> {
        self.kept
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    Attr::Str(s) => Value::from(s.as_str()),
                    Attr::I64(n) => Value::from(*n),
                    Attr::F64(n) => Value::from(*n),
                    Attr::Bool(b) => Value::from(*b),
                };
                ((*name).to_owned(), value)
            })
            .collect()
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.string(field, value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.number(field, Attr::I64(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.number(field, Attr::I64(i64::try_from(value).unwrap_or(i64::MAX)));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.number(field, Attr::F64(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.number(field, Attr::Bool(value));
    }

    fn record_error(&mut self, _field: &Field, _value: &(dyn std::error::Error + 'static)) {
        // Error messages are free text.
        self.redacted += 1;
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            // Messages are free text and are never exported.
            return;
        }
        self.string(field, &format!("{value:?}"));
    }
}

/// Per-span state kept in the registry's extensions.
struct SpanState {
    cx: Option<Context>,
    name: &'static str,
    /// The span's allowed fields, repeated on its events' log lines.
    fields: Map<String, Value>,
    redacted: u32,
}

/// The layer that exports spans to an OpenTelemetry tracer and events as JSON log lines.
pub struct ExportLayer {
    tracer: Option<SdkTracer>,
    logs: Arc<dyn LogSink>,
}

impl ExportLayer {
    /// Exports spans to `tracer` (none: logs only) and log lines to `logs`.
    pub fn new(tracer: Option<SdkTracer>, logs: Arc<dyn LogSink>) -> Self {
        Self { tracer, logs }
    }
}

impl<S> Layer<S> for ExportLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: LayerContext<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        let cx = self.tracer.as_ref().map(|tracer| {
            let parent = span
                .parent()
                .and_then(|p| p.extensions().get::<SpanState>().and_then(|s| s.cx.clone()))
                .unwrap_or_default();
            let builder = SpanBuilder::from_name(attrs.metadata().name())
                .with_start_time(SystemTime::now())
                .with_attributes(fields.key_values());
            parent.with_span(tracer.build_with_context(builder, &parent))
        });
        span.extensions_mut().insert(SpanState {
            cx,
            name: attrs.metadata().name(),
            fields: fields.json(),
            redacted: fields.redacted,
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: LayerContext<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut fields = Fields::default();
        values.record(&mut fields);
        if let Some(state) = span.extensions_mut().get_mut::<SpanState>() {
            state.redacted += fields.redacted;
            state.fields.extend(fields.json());
            if let Some(cx) = &state.cx {
                cx.span().set_attributes(fields.key_values());
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: LayerContext<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let metadata = event.metadata();
        let mut line = Map::new();
        line.insert("ts".into(), Value::from(jiff::Timestamp::now().to_string()));
        line.insert("level".into(), Value::from(metadata.level().as_str()));
        line.insert("target".into(), Value::from(metadata.target()));
        if let Some(lineno) = metadata.line() {
            line.insert("line".into(), Value::from(lineno));
        }
        if let Some(span) = ctx.event_span(event)
            && let Some(state) = span.extensions().get::<SpanState>()
        {
            line.insert("span".into(), Value::from(state.name));
            if !state.fields.is_empty() {
                line.insert("span_fields".into(), Value::Object(state.fields.clone()));
            }
            if let Some(cx) = &state.cx {
                let span_context = cx.span().span_context().clone();
                line.insert(
                    "trace_id".into(),
                    Value::from(span_context.trace_id().to_string()),
                );
                line.insert(
                    "span_id".into(),
                    Value::from(span_context.span_id().to_string()),
                );
                let mut attributes = fields.key_values();
                attributes.push(KeyValue::new("level", metadata.level().as_str()));
                attributes.push(KeyValue::new("target", metadata.target()));
                cx.span().add_event(metadata.name(), attributes);
            }
        }
        line.insert("fields".into(), Value::Object(fields.json()));
        if fields.redacted > 0 {
            line.insert(
                "telemetry.redacted_fields".into(),
                Value::from(fields.redacted),
            );
        }
        self.logs.write(&Value::Object(line).to_string());
    }

    fn on_close(&self, id: Id, ctx: LayerContext<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        if let Some(state) = span.extensions_mut().remove::<SpanState>()
            && let Some(cx) = state.cx
        {
            if state.redacted > 0 {
                cx.span().set_attribute(KeyValue::new(
                    "telemetry.redacted_fields",
                    i64::from(state.redacted),
                ));
            }
            cx.span().end();
        }
    }
}
