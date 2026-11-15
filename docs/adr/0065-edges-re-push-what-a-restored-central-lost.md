# Edges re-push what a restored central lost, from the acknowledged point central's backup kept

> Amends [ADR 0019](0019-own-sync-protocol-with-commit-ordered-cursors.md) and [ADR 0046](0046-central-recovery-by-hourly-export-and-warm-standby.md).

ADR 0046 says a restored central gets a new epoch and "each edge re-sends from its own checkpoints". Nothing did that. An edge removes a group from its outbox once central acknowledges it. A record central acknowledged and then lost in a restore, up to an hour of pushes, was never sent again. The new epoch only made the edge re-seed its pull side. Central also kept no record of how far it held each edge's log. The maintainer chose this mechanism (2026-11-14, #138).

**The decision:**
- **The edge says what was acknowledged.** Each `Hello` carries `pushed`: the last `seq` of the edge's own log the upstream has acknowledged, in the edge's current epoch. The edge records it when it records a push's answers (`sync_upstream.pushed`).
- **Central keeps it with the handshake.** Central already stores each peer's last `Hello` (`sync_peer`). That record is part of central's backup, so after a restore it holds what the edge had been acknowledged at the last handshake before the backup. It can only be at or behind what the restored store holds, never ahead.
- **Central answers with it.** Before storing a new `Hello`, central reads the stored one. If it was in the same edge epoch, its `pushed` becomes the `Welcome`'s `expected` cursor. The `Welcome` also carries central's own log `epoch`.
- **The edge re-queues on a new central epoch.** When the `Welcome`'s epoch differs from the one the edge saw last (`sync_upstream.upstream_epoch`), central was restored. The edge queues again every entry on its log after `expected` (all of it, without one) that it would have queued for central: not those central sent it, and not down-only ones. Then it pushes as usual, before pulling. The first handshake only notes the epoch, because adding the upstream already queued the backlog. The alert is `sync-repush`.
- **Duplicates are harmless.** Anything central still holds is answered from its receipts, or written as nothing because the version is present. Re-pushing a little too much is the safe direction.
- **The same holds under an edge.** The edge's own sync server runs the same code for its desktops.
- **The rate limit** (ADR 0046; #138, part 3) keeps the burst of re-pushes from every edge queued.

## Considered Options

| Option | Decision |
|---|---|
| **The edge reports what was acknowledged, and central keeps it with the handshake it already stores** | **Chosen**: one optional field each way, no new table at central, and no clock in the decision |
| Central keeps its own per-edge watermark, written with its receipts | Rejected: every pushed item would need its sender's `seq`, changing the envelope for one number per push |
| Central announces its restore point as a time, and edges re-queue what they committed after it, minus a margin | Rejected: depends on clocks agreeing, and the margin re-sends an arbitrary amount |
| Edges re-push their whole log after a central restore | Rejected: every facility's full history arrives at once, when only an hour was lost |

## Consequences

- **The protocol** gains optional fields: `Hello.pushed` and `Welcome.epoch`. `Welcome.expected` is filled in for the first time. Peers that send neither keep today's behaviour.
- **Migration `0058_sync_repush`** adds `pushed` and `upstream_epoch` to `sync_upstream`.
- **Lag.** The stored `pushed` lags by up to a round, because it comes from the previous handshake. So a restore re-pushes at most one round more than was lost.
- **Not covered.**
  - Entries already pruned from the edge's log below `expected` cannot be re-pushed. The log horizon is far longer than central's backup interval, so a normal restore never reaches them.
  - Break-glass grants, transfers and activation reports have their own queues. Re-sending those after a restore is later work.
- **`health-sync-sim`** restores a node from an exported backup (`Node::restore_from`), so its scenarios lose data as a real restore does. `tests/restore.rs` checks that the lost hour comes back from the edge.
