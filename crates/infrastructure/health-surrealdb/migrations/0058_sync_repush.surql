-- Re-pushing what a restored upstream lost (#138; ADR 0065).
-- - `sync_upstream.pushed`: the last `seq` of this node's log the upstream has acknowledged,
--   sent in each Hello so the upstream keeps it with the handshake.
-- - `sync_upstream.upstream_epoch`: the upstream's log epoch at the last handshake. When it
--   changes, the upstream was restored, and this node re-queues its log after what the upstream
--   says it still holds.
DEFINE FIELD pushed ON sync_upstream TYPE option<int>;
DEFINE FIELD upstream_epoch ON sync_upstream TYPE option<string>;
