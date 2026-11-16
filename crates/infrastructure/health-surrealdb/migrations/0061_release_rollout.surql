-- Staged software roll-out (#136; deployment.md § Environments and rollout; ADRs 0041, 0045).
-- Platform releases imported at central: each version's signed artifacts, kept in central's blob
-- store, with their digests and signatures as JSON. Imported once and never changed.
DEFINE TABLE platform_release SCHEMAFULL;
DEFINE FIELD imported_at ON platform_release TYPE datetime;
DEFINE FIELD data ON platform_release TYPE string;
-- Roll-out records: issued at central, sent down to every node as operations, never changed. A
-- later record in a scope replaces an earlier one. Each is kept as JSON, keyed by its id.
DEFINE TABLE release_rollout SCHEMAFULL;
DEFINE FIELD issued_at ON release_rollout TYPE datetime;
DEFINE FIELD data ON release_rollout TYPE string;
DEFINE INDEX release_rollout_issued ON release_rollout FIELDS issued_at;
