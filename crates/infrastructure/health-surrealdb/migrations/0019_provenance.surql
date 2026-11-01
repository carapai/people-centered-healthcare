-- Provenance (#196; resource-ownership.md rule 7): one table, each row owned by its target's
-- Owner. `owner` is stamped from the target record, read in the same transaction, so category
-- partitions apply; sync selects a context's Provenance by it, so it travels with its target.
DEFINE TABLE provenance SCHEMAFULL;
DEFINE FIELD json ON provenance TYPE string;
DEFINE FIELD version_id ON provenance TYPE string;
DEFINE FIELD version_seq ON provenance TYPE int;
DEFINE FIELD last_updated ON provenance TYPE datetime;
DEFINE FIELD deleted ON provenance TYPE bool;
DEFINE FIELD index_keys ON provenance TYPE array<record> DEFAULT [];
DEFINE FIELD owner ON provenance TYPE string;
DEFINE INDEX provenance_owner ON provenance FIELDS owner;
DEFINE TABLE provenance_history SCHEMAFULL;
DEFINE FIELD json ON provenance_history TYPE string;
DEFINE FIELD version_id ON provenance_history TYPE string;
DEFINE FIELD version_seq ON provenance_history TYPE int;
DEFINE FIELD last_updated ON provenance_history TYPE datetime;
DEFINE FIELD deleted ON provenance_history TYPE bool;
DEFINE FIELD owner ON provenance_history TYPE string;
DEFINE INDEX provenance_history_last_updated ON provenance_history FIELDS last_updated, version_seq;
