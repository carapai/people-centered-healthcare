# ops/observability

The in-country observability stack (ADR 0047; [deployment.md § Observability](../../docs/architecture/deployment.md#observability-adr-0047); #139): an OpenTelemetry collector, Prometheus, Loki, Tempo, Alertmanager and Grafana, each image pinned.

```sh
CENTRAL_LOG_DIR=/var/log/central docker compose -f ops/observability/docker-compose.yml up -d
```

- **Edges** ship their buffered log records and spans to the collector: `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT=http://<collector>:4318/v1/logs` and `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://<collector>:4318/v1/traces`.
- **Central** writes JSON log lines to stdout. The collector reads its log files from `CENTRAL_LOG_DIR`.
- **Grafana** (port 3000) holds the *Health platform: indicators* dashboard, provisioned from `grafana/`.

## Alerts

Every node logs its alerts with an allowlisted `alert` code, raised and later cleared (`outcome = "cleared"`). [`alerts.tsv`](alerts.tsv) catalogues every code with its indicator area, severity and summary.

- **One rule per code.** [`loki/rules/fake/alerts.yaml`](loki/rules/fake/alerts.yaml) is generated from the catalogue. A rule fires while its code has been raised and not cleared within the last 24 hours, per node (`service_instance_id`) and subject (`check`). A one-off event, such as a payment exception, stays firing for 24 hours.
- **Routing.** Alertmanager sends every alert to central operations. The webhook in [`alertmanager/alertmanager.yml`](alertmanager/alertmanager.yml) is a placeholder, to be replaced with operations' paging address. Alerts are also shown on the edge's admin page, as before.
- **Coverage check.** [`check-alerts.sh`](check-alerts.sh), run from the repository root, fails if a code is emitted but not catalogued, catalogued but never emitted, or catalogued without a rule.

## No personal health data

The nodes export only allowlisted fields (`health-observability`). The collector enforces the allowlist again, so a node on an older release cannot leak a field:

- every log body is cleared, since event messages are never exported, and a file-read line would otherwise keep the raw line as its body;
- every attribute not on the allowlist is dropped (`transform/allowlist`).

## Verified (2026-11-13, locally, with these images)

- **Valid configs:** `otelcol validate`, `promtool check config`, `amtool check-config` and `loki -verify-config` all pass.
- **The stack, end to end:**
  - An edge-shaped OTLP record with `alert = "ups-on-battery"` raises `UpsOnBattery` in Alertmanager, labelled with the edge's node. Its `outcome = "cleared"` record resolves it.
  - A central-style JSON line with `alert = "sync-failing"` raises `SyncFailing` for `central-server`.
- **Privacy:**
  - A `patient_name` attribute never reached Loki.
  - A `phone` field never reached Loki, as an attribute or in the body.
- **Still to check:** the dashboard has not been rendered in Grafana; port 3000 was taken on the test machine. Its queries follow the same Loki selectors as the rules.

## Indicators without an alert code yet

deployment.md lists some indicators that no node logs a code for yet:

- **Sync:** lag and outbox depth (as numbers);
- **Finance:** pending-payment age;
- **Stock:** drift;
- **Workflow:** overdue timers and queue wait times;
- **Platform:** the software version against the allowed version.

Each needs a node to log it (an allowlisted `_lag`, `_depth`, `_seconds` or `_count` field, or a code); then a row in `alerts.tsv` and a regenerated rule.

## Left to operations

- Standing up the stack on the in-country site.
- Sizing and retention. Staging keeps logs and metrics 90 days and traces 14.
- The paging address, and who is on call.
- Watching each alert fire in staging, the task's acceptance.
