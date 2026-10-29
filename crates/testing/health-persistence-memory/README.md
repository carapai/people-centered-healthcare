# health-persistence-memory

Layer: **testing** (dev-dependency only). `MemoryStore` implements `health-persistence`.

- **Isolation.** Each transaction reads a snapshot taken at `begin`.
- **Commits.** The first committer wins: touching a head, guard or counter that a later commit changed fails with `WriteError::Conflict`.
- **Inspection.** Tests can read `outbox()` and `history()`.

`tests/memory_store.rs` exercises the contract: preconditions, stamping, rollback, conflicts, tombstones, guards, outbox atomicity, and 8 threads × 200 concurrent sequence allocations with no duplicates or gaps.
