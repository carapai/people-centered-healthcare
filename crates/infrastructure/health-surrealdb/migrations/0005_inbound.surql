-- The Inbound Dead Letter and the quarantine queue (ADR 0051; fhir-r5-model.md § Validation).
-- External payloads that fail validation are kept here, never dropped: a tier 1 failure keeps
-- the raw payload as a dead letter; a tier 2-4 failure, or a write the store refuses, keeps the
-- resource in quarantine for review. The integration module owns the table; every access is
-- audited, and entries are purged when their integration's retention ends.
DEFINE TABLE inbound SCHEMAFULL;
DEFINE FIELD kind ON inbound TYPE string
  ASSERT $value IN ['dead_letter', 'quarantine'];
DEFINE FIELD integration ON inbound TYPE string;
DEFINE FIELD received_at ON inbound TYPE datetime;
DEFINE FIELD retain_until ON inbound TYPE datetime;
-- The payload exactly as received; for a dead letter it may not even be JSON.
DEFINE FIELD payload ON inbound TYPE string;
-- One "expression: code: diagnostics" line per problem, never the payload's values.
DEFINE FIELD issues ON inbound TYPE array<string>;
DEFINE FIELD status ON inbound TYPE string
  ASSERT $value IN ['pending', 'replayed', 'discarded'];
DEFINE FIELD resolved_at ON inbound TYPE option<datetime>;
DEFINE FIELD note ON inbound TYPE option<string>;
DEFINE INDEX inbound_review ON inbound FIELDS kind, status, received_at;
DEFINE INDEX inbound_retention ON inbound FIELDS retain_until;
