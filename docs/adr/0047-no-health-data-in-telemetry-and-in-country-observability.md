# Telemetry never carries personal health data, and the observability stack is self-hosted in-country

**Collection.** Central, edges, desktops and frontend error reporting all emit OpenTelemetry. It goes to a self-hosted, in-country stack: an OTel collector, Prometheus-compatible metrics, Loki logs, Tempo traces and Grafana.

**Allowlist.** A tracing layer enforces an attribute allowlist, and a CI redaction test checks it.
- **Allowed:** resource UUIDs, node and facility ids, operation names, codes from non-sensitive ValueSets, durations and counts.
- **Never allowed:** names, phone numbers, NINs, free text, or codes from sensitive ValueSets (HIV, SRH, mental health).

**Indicators per node:**
- sync lag, outbox depth, and open conflicts and rejections with their age;
- backup freshness and verification;
- free disk, UPS state, and clock skew against the clock floor;
- expiry of certificates and permission snapshots;
- age of pending payments, reconciliation exceptions, stock drift, and stuck workflow timers.

**Alerts** go to central operations and are shown on the edge admin page.

We chose this because logs and traces are copied, retained and viewed far more widely than clinical records. Uganda's DPPA also makes cross-border transfer of personal data a compliance question.

## Considered Options

- **A SaaS observability vendor.** Rejected: data would leave the country, and edges are offline for long periods anyway.
- **Redaction only at the collector.** Rejected: data would already have left the node, and edge buffers would hold it.

## Consequences

- Debugging uses resource UUIDs. Seeing the clinical content behind them needs normal authorized access to the record.
