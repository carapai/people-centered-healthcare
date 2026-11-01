//! Shipping the telemetry buffer to the in-country collector over OTLP/HTTP (JSON), oldest
//! segment first. A segment is removed only once the collector accepts it, so lines buffered
//! offline are shipped when the node is online again.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use health_time::Timestamp;
use serde_json::{Map, Value, json};

use super::buffer::Buffer;
use super::report::Telemetry;

/// How long one batch may take.
const POST_TIMEOUT: Duration = Duration::from_secs(30);

/// The shipper's last outcome, for the status page.
#[derive(Debug, Clone, Default)]
pub(crate) struct Shipping {
    pub(crate) configured: bool,
    pub(crate) last_shipped: Option<Timestamp>,
    pub(crate) failing: bool,
}

/// [`Shipping`], shared between the shipper and the status page.
pub(crate) type ShipState = Arc<Mutex<Shipping>>;

impl Shipping {
    pub(crate) fn telemetry(state: &ShipState, buffer: Option<&Buffer>) -> Telemetry {
        let shipping = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let (buffered_bytes, dropped_bytes) = buffer.map_or((0, 0), Buffer::stats);
        Telemetry {
            buffered_bytes,
            dropped_bytes,
            shipping: shipping.configured,
            last_shipped: shipping.last_shipped,
            failing: shipping.failing,
        }
    }
}

/// Ships `buffer` to `endpoint` every `every`, until the node stops.
pub(crate) async fn ship(
    buffer: Arc<Buffer>,
    endpoint: String,
    node: String,
    every: Duration,
    state: ShipState,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(POST_TIMEOUT)
        .build()
        .map_err(|_| "cannot build the telemetry client".to_owned())?;
    loop {
        let shipped = round(&client, &buffer, &endpoint, &node).await;
        let failing = shipped.is_err();
        let changed = {
            let mut shipping = state.lock().unwrap_or_else(|e| e.into_inner());
            if shipped == Ok(true) {
                shipping.last_shipped = Some(Timestamp::now());
            }
            std::mem::replace(&mut shipping.failing, failing) != failing
        };
        if changed && failing {
            tracing::warn!(alert = "telemetry-unshipped", "cannot ship telemetry");
        } else if changed {
            tracing::info!(
                alert = "telemetry-unshipped",
                outcome = "cleared",
                "telemetry shipping"
            );
        }
        tokio::time::sleep(every).await;
    }
}

/// Ships every sealed segment, oldest first, stopping at the first the collector refuses.
/// `Ok(true)` when something was shipped.
async fn round(
    client: &reqwest::Client,
    buffer: &Buffer,
    endpoint: &str,
    node: &str,
) -> Result<bool, ()> {
    let mut shipped = false;
    for segment in buffer.seal() {
        let Ok(text) = tokio::fs::read_to_string(&segment).await else {
            continue;
        };
        let body = otlp(&text, node);
        let accepted = client
            .post(endpoint)
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .is_ok_and(|r| r.status().is_success());
        if !accepted {
            return Err(());
        }
        buffer.shipped(&segment);
        shipped = true;
    }
    Ok(shipped)
}

/// An OTLP `ExportLogsServiceRequest`, in its JSON encoding, for the buffered `lines`.
pub(crate) fn otlp(lines: &str, node: &str) -> Value {
    let records: Vec<Value> = lines
        .lines()
        .filter_map(|line| serde_json::from_str::<Map<String, Value>>(line).ok())
        .map(record)
        .collect();
    json!({
        "resourceLogs": [{
            "resource": {"attributes": [
                attribute("service.name", &Value::from("facility-edge")),
                attribute("service.instance.id", &Value::from(node)),
            ]},
            "scopeLogs": [{
                "scope": {"name": "health-edge-runtime"},
                "logRecords": records,
            }],
        }],
    })
}

fn record(line: Map<String, Value>) -> Value {
    let nanos = line
        .get("ts")
        .and_then(Value::as_str)
        .and_then(|ts| ts.parse::<Timestamp>().ok())
        .map_or(0, |ts| ts.as_nanosecond());
    let level = line.get("level").and_then(Value::as_str).unwrap_or("INFO");
    let mut attributes = Vec::new();
    for key in ["target", "span"] {
        if let Some(value) = line.get(key) {
            attributes.push(attribute(key, value));
        }
    }
    for group in ["span_fields", "fields"] {
        if let Some(Value::Object(fields)) = line.get(group) {
            attributes.extend(fields.iter().map(|(k, v)| attribute(k, v)));
        }
    }
    if let Some(count) = line.get("telemetry.redacted_fields") {
        attributes.push(attribute("telemetry.redacted_fields", count));
    }
    let mut record = json!({
        "timeUnixNano": nanos.to_string(),
        "severityText": level,
        "severityNumber": severity(level),
        "attributes": attributes,
    });
    for (from, to) in [("trace_id", "traceId"), ("span_id", "spanId")] {
        if let Some(id) = line.get(from) {
            record[to] = id.clone();
        }
    }
    record
}

fn attribute(key: &str, value: &Value) -> Value {
    let value = match value {
        Value::Bool(b) => json!({"boolValue": b}),
        Value::Number(n) if n.is_i64() || n.is_u64() => json!({"intValue": n.to_string()}),
        Value::Number(n) => json!({"doubleValue": n}),
        Value::String(s) => json!({"stringValue": s}),
        other => json!({"stringValue": other.to_string()}),
    };
    json!({"key": key, "value": value})
}

/// OTLP's severity numbers for `tracing`'s levels.
fn severity(level: &str) -> u8 {
    match level {
        "TRACE" => 1,
        "DEBUG" => 5,
        "WARN" => 13,
        "ERROR" => 17,
        _ => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffered_lines_become_otlp_log_records() {
        let lines = concat!(
            r#"{"ts":"2026-11-02T09:00:00Z","level":"WARN","target":"health_edge_runtime::admin","fields":{"alert":"disk-full","check":"data"},"trace_id":"0af7651916cd43dd8448eb211c80319c"}"#,
            "\n",
            "not json\n",
            r#"{"ts":"2026-11-02T09:00:01Z","level":"INFO","target":"t","span":"indicators","span_fields":{"store_bytes":42},"fields":{"alert_count":1}}"#,
            "\n",
        );
        let body = otlp(lines, "facility-edge");
        let resource = &body["resourceLogs"][0];
        assert_eq!(
            resource["resource"]["attributes"][1],
            json!({"key": "service.instance.id", "value": {"stringValue": "facility-edge"}})
        );
        let records = resource["scopeLogs"][0]["logRecords"]
            .as_array()
            .expect("records");
        assert_eq!(records.len(), 2, "a line that is not JSON is skipped");
        assert_eq!(records[0]["timeUnixNano"], "1793610000000000000");
        assert_eq!(records[0]["severityNumber"], 13);
        assert_eq!(records[0]["traceId"], "0af7651916cd43dd8448eb211c80319c");
        assert!(
            records[0]["attributes"]
                .as_array()
                .expect("attributes")
                .contains(&json!({"key": "alert", "value": {"stringValue": "disk-full"}}))
        );
        assert!(
            records[1]["attributes"]
                .as_array()
                .expect("attributes")
                .contains(&json!({"key": "store_bytes", "value": {"intValue": "42"}}))
        );
    }
}
