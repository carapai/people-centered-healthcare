# Edge status

The edge admin status page (facility-edge.md § Health monitoring; #77).

- **`StatusPage`** (`/edge/status`, linked from the home page on an edge): the report from `GET /api/admin/status`, refreshed every 30 seconds.
  - **Alerts:** disk over 80%, backups off, failed or older than two intervals, a failed test restore, the UPS on battery, low or unreachable, and a task that keeps restarting.
  - **Indicators:** node ID, version and uptime; database size and free space on the data and backup disks; the last backup and test restore; the UPS; the telemetry waiting for central and when it was last sent; task restarts.
  - **Not reported yet:** the report lists them (sync, clock skew, certificates, the permission snapshot, disk encryption, sessions); each arrives with its feature.
- **On the server:** `health_edge_runtime::admin`. Until the edge's local authorization server (#75) can require an admin, the edge answers only requests from the node itself, and the page explains a 403. Central has no such route, so the page says it is for an edge.
- The report holds no personal health data.

## Tests

- `test/edge-status.test.ts`: the API and the byte and percentage formatting.
