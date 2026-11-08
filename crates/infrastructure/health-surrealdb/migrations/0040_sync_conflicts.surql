-- Conflict Records and their review (#82; synchronization.md § Conflict Record and review queue;
-- ADR 0020): a record keeps both branches, the candidate's items as sent, with the class of the
-- records and its policy, and once reviewed, who resolved it, when, and the resulting version.
DEFINE FIELD candidates ON sync_conflict TYPE option<string>;
DEFINE FIELD class ON sync_conflict TYPE option<string>;
DEFINE FIELD policy ON sync_conflict TYPE option<string>;
DEFINE FIELD resolved_by ON sync_conflict TYPE option<string>;
DEFINE FIELD resolved_at ON sync_conflict TYPE option<datetime>;
DEFINE FIELD resolution ON sync_conflict TYPE option<string>;
DEFINE INDEX sync_conflict_status ON sync_conflict FIELDS status, opened_at;
