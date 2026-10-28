# Central recovers from hourly exports to a warm standby, and edges re-push facility data after a central restore

**Central v1** runs a single SurrealDB node, without Enterprise high availability.
- **Backups:** a snapshot-consistent logical export every hour.
- **Standby:** a warm standby VM at a second in-country site is restored from the latest export.
- **Global records** (registries, metadata, identity, permissions): **RPO 1 h, RTO 4 h**.
- **Facility records:** losses within the RPO are recovered by the sync design. A restored central has a new epoch, so each edge re-sends from its own checkpoints (ADR 0019). Edges keep their authoritative records locally.

**Edges and mode B desktops** keep their targets: RPO of one backup interval (1 h) for unsynced work, and RTO under 1 h with a spare (ADR 0007).

**Retention:**

| Node | Copies kept |
|---|---|
| Central | 35 daily, 12 weekly, 12 monthly, plus an offsite in-country copy |
| Edge | 48 hourly, 14 daily |

**Keys.** Per-node backup keys are wrapped and escrowed in central KMS. Central's backup master key uses 3-of-5 Shamir custody, with the custodians named in the operations runbook.

**Drills:**
- a central restore into staging every quarter, with data-integrity validation;
- a weekly automatic test restore by each edge into a temporary directory;
- a facility restore drill at commissioning, then yearly.

We chose this because SurrealDB Community has no physical hot backup, point-in-time recovery or multi-node HA. Facility data, however, is already replicated by the edges, so central's real exposure is limited to global records.

## Considered Options

- **SurrealDB Enterprise or TiKV high availability now.** Deferred: licensing cost and the BSL questions stay in the legal fog, and TiKV in Community is experimental.
- **More frequent exports, e.g. every 5 minutes.** Rejected for v1: a full logical export is a long read transaction at national scale. This can be revisited with incremental exports.

## Consequences

- An outage of central blocks only central-only functions. Facilities keep working offline.
- Restoring central causes a burst of re-sync traffic from edges, so the sync servers must rate-limit.
