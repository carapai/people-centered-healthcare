# health-observability

Layer: **infrastructure**. Telemetry with an attribute allowlist (ADR 0047; deployment.md § Observability).

- **`init(&TelemetryConfig)`** installs the process subscriber: an `EnvFilter` and the `ExportLayer`. Spans go over OTLP/HTTP in batches when an endpoint is configured. Events go to stdout as JSON lines for the node's collector.
- **`init_with(&TelemetryConfig, logs)`** does the same with another `LogSink` for the log lines: the edge's bounded buffer, shipped when online (`health_edge_runtime::admin`).
- **`ExportLayer`** is the only exporter. It builds OpenTelemetry spans and log lines itself from fields that pass the **allowlist** (`allowlist::shape`, `allowlist::admits`):
  - **Name:** a field must be named on the allowlist (`resource_id`, `operation`, `http.route`, `duration_ms`, the edge's `alert` and `state`, …).
  - **Value:** its value must have that name's shape:
    - a UUID for `*_id`;
    - a short identifier with no run of seven or more digits;
    - a route template;
    - a code;
    - a number.
  - **Dropped:** anything else, counted in `telemetry.redacted_fields`.
  - **Never exported:** event messages, error values and `?Debug` values, because they are free text. Log lines repeat the enclosing span's allowed fields.
- **Writing telemetry:** record identifiers and measurements as fields, e.g. `info!(resource_id = %id, duration_ms, "read")`. The message is for the reader of the code only. Never record codes from sensitive ValueSets.

`tests/redaction.rs` records names, phone numbers, NINs and free text in every way `tracing` allows and checks that none of them reaches an exported span or log line. A negative control (keeping every string) makes it fail.
