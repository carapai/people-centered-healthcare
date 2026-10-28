# Backup and recovery

Status: **accepted** (ADR 0007). Produced while resolving the Wayfinder ticket [What is the Facility Edge Node's process, storage and recovery model?](https://github.com/carapai/people-centered-healthcare/issues/18). This page covers the Facility Edge Node; process and update rules are in [facility-edge.md](facility-edge.md). Central recovery targets, retention, key custody and drills are in [deployment.md](deployment.md) (ADR 0046); mode B desktops follow this page (desktop-client.md).

## Backup

Recovery draws on two sources. **Central** holds everything already synced. **Local backups** cover the window that has not been uploaded, and any outage.

- **What:** a snapshot-consistent logical export, taken through the SurrealDB SDK inside the edge process. It includes the outbox, so un-uploaded work is preserved. The content-addressed blob directory (ADR 0021) is copied alongside it, adding only new hashes.
- **When:** hourly while the facility is active. Retention is 48 hourly and 14 daily copies. An admin can also run it on demand, including to USB. One is taken automatically before every update.
- **Where:** the second volume, never the data volume.
- **Protection:** every backup is encrypted with a per-node backup key. The key is wrapped and escrowed in central KMS (custody in [deployment.md](deployment.md), ADR 0046).
- **Integrity:** every backup is verified after it is written, using its checksum and manifest. CI runs an export→import round-trip test on representative data, and the schema avoids `set<record>` values because of SurrealDB bug #7621.

## Restore and failover

- **No hot standby and no automatic failover** in v1 (ADR 0007).
- **Same machine, corrupted store:** stop the node, restore the latest verified backup, start, and resume sync.
- **Machine lost:** use a cold spare, either pre-enrolled or any baseline PC. First re-provision it from central (node identity re-issued, facility data pulled down). Then import the un-uploaded outbox operations from the last local backup. Sync deduplicates them by `op_id`.
- **Targets:** data loss of at most one backup interval for work not yet synced, and under one hour to recover when a spare is available. Formal targets and drills are in [deployment.md](deployment.md) (ADR 0046).
