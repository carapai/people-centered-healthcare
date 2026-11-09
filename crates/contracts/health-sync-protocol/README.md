# health-sync-protocol

Layer: **contracts**. The `sync/v1` protocol (synchronization.md; ADRs 0019, 0020, 0061; #80): the wire types and the rules both sides share, with no I/O. `health-sync-server` at central and the edge and desktop sync modules use it, so every hop runs the same code.

- **`Item`:** the envelope for every version or ledger operation.
  - `check()` verifies that `content_hash` matches the body and that the subject fits the kind.
  - **`ContentHash`** is the sha-256 of canonical JSON: keys sorted, no whitespace.
  - **`Hlc`** orders items for audit only.
- **Handshake:**
  - **`Compatibility::judge`** turns a `Hello` into a `Verdict`. A different protocol major is refused. Software outside N-1, or an unapproved package, is quarantined: the peer may push but receives nothing.
  - **`expected_cursor`** drops a checkpoint from another node or epoch.
- **Push:**
  - **`group`** splits items read in `seq` order into transactions.
  - **`batches`** packs groups into batches of at most 500 items or 4 MiB, never splitting a group.
- **Receiver:** **`receive`** handles one group over the **`Authority`** port.
  - A group sent before is answered from its `Receipt`s and never applied twice.
  - An `op_id` reused with another hash is rejected (`op_id_reused`).
  - A malformed or partly known group is rejected without applying.
- **Pull:**
  - **`decide`** never applies an item twice, and never overwrites an unaccepted local candidate: the pulled version becomes the head and the candidate is kept.
  - **`apply_pull`** works over the **`Replica`** port and applies one group per transaction, each with its checkpoint.
- **`ItemState`:** pending, sending, accepted, conflict, rejected, held, and the transitions between them.
- **`to_ndjson` / `from_ndjson`:** request and response bodies.
- **`compress` / `decompress`:** the bodies' zstd encoding on the wire, refusing a body that expands past `MAX_DECODED` (64 MiB).
