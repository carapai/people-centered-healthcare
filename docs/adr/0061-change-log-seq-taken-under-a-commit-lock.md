# The change log's seq is taken under the node's commit lock

This amends ADR 0019. A node still numbers every accepted write in commit order, so a cursor never skips a write that commits late. What changes is how: **an in-process commit lock** replaces the counter record written inside every write transaction.

- **The rule.** A unit of work that wrote versions takes the node's commit lock when it commits. Holding it, the unit of work:
  1. reads the last committed `seq` outside its own snapshot;
  2. numbers its `change_log` entries after it;
  3. writes them and the upstream outbox rows in its transaction;
  4. commits, then releases the lock.

  A transaction that wrote nothing commits without the lock.
- **Why this is still commit order.** Only the lock holder can take a `seq`, and it commits before anyone else can take one. A reader that has seen `seq` n therefore never later finds an entry below n.
- **Why it is safe.** One process writes each database: the edge and the standalone desktop are embedded and single-process (ADR 0007), and central is one `central-server` (deployment-topology.md); its warm standby is not active at the same time. The lock lives on the shared `Connection`, so every unit of work in that process takes the same one.
- **The epoch** stays a record (`sync_state:node`). It is written only when the node is restored from backup or re-seeded.

We chose this because the counter record made every pair of overlapping write transactions conflict, not only writers of the same resource. Measured on 4.0.0 (in-memory engine, debug build, 8 concurrent writers, 200 writes), the counter record caused 791 conflict retries, about 4 per write. That is beyond `RetryPolicy`'s budget of 4 attempts, and the 200 writes took about 60 s. Under the lock, the same test retried 0 conflicts. Writes still serialize, as ADR 0019 accepted, but they wait in a queue rather than failing and rerunning their whole use case.

## Considered Options

| Option | Decision |
|---|---|
| A counter record in every write transaction (ADR 0019 as written) | Rejected: every overlapping writer conflicts and reruns; the retry budget is exceeded at 8 writers |
| Per-writer sub-counters merged by a stable-horizon reader (spike #36's fallback) | Rejected for now: more moving parts, and readers lag by the horizon |
| An in-process commit lock, `seq` = last committed + 1 | **Chosen** |

## Consequences

- **Single writer per database is now a requirement**, no longer just how nodes are deployed. A second process writing the same database (a script, or a second `central-server` against the same SurrealDB) could commit `seq`s out of order. Two writers would race to create the same `change_log` record, and one of them would fail. Tools that write records go through the node's own process.
- **Throughput** is bounded by one commit at a time: the commit's latency, including `sync=every`, sets the node's write rate. Spike #36 now measures that on edge hardware, in place of the counter's conflict rate.
- **The commit lock is held only for the append and the commit**, never while a use case reads or validates.
