# Facility Edge is one process embedding SurrealDB, recovered by cold spare

Each facility runs exactly one Facility Edge Node in v1. It is a single `facility-edge` process that embeds SurrealDB (RocksDB, `sync=every`) and runs every background job as a supervised task. Recovery uses local encrypted logical backups plus central, restored onto a cold spare. There is no hot standby and no automatic failover. SurrealDB holds an exclusive lock on its data directory and the Community edition has no replication. A second writer inside the LAN would therefore need our own LAN-internal conflict handling, which is not justified for v1. Embedding avoids a second service, a second credential surface and a localhost database port, and it matches the Tauri desktop storage path.

## Considered Options

- **SurrealDB as a sibling `surreal start` service.** This keeps the `surreal` CLI and SurrealKit usable while the node runs. Rejected because it means two services to supervise and secure, for tooling we replace with the edge's own admin commands.
- **Hot standby or multiple edges per facility.** Deferred to the map's Not yet specified. Either one needs edge-to-edge replication and a write-authority split inside a facility.

## Consequences

- The recovery point objective at the facility is bounded by the hourly backup, apart from what has already synced to central. Recovery takes under an hour if a spare machine is ready.
- Schema migrations and backups run inside the edge process. External tools must never open the data directory while the node runs.
- A power-cut validation spike on Windows NTFS and Linux ext4 must pass before edge storage is implemented.
