-- Illustrative sketch for docs/architecture/persistence.md. Not a migration.
-- Validated with `surreal validate` (CLI 3.2.4); exact range bounds are verified by EXPLAIN tests in implementation.
-- Current (head) table for one resource type
DEFINE TABLE observation SCHEMAFULL;
DEFINE FIELD resource ON observation TYPE object FLEXIBLE;
DEFINE FIELD version_id ON observation TYPE uuid;
DEFINE FIELD version_seq ON observation TYPE int;
DEFINE FIELD last_updated ON observation TYPE datetime;
DEFINE FIELD deleted ON observation TYPE bool DEFAULT false;
DEFINE FIELD facility ON observation TYPE option<record<organization>>;
DEFINE FIELD authority_node ON observation TYPE uuid;
DEFINE FIELD provisional ON observation TYPE bool DEFAULT false;
DEFINE FIELD security_labels ON observation TYPE array<string>;
DEFINE FIELD profiles ON observation TYPE array<string>;
DEFINE FIELD index_keys ON observation TYPE array<record>;
-- hot projections
DEFINE FIELD patient ON observation TYPE option<record<patient>>;
DEFINE FIELD encounter ON observation TYPE option<record<encounter>>;
DEFINE FIELD code ON observation TYPE array<string>;
DEFINE FIELD effective_at ON observation TYPE option<datetime>;
DEFINE FIELD status ON observation TYPE string;
DEFINE INDEX observation_patient_code_time ON observation FIELDS patient, code, effective_at;

-- History: id = [resource_uuid, version_seq]
DEFINE TABLE observation_history SCHEMAFULL;
DEFINE FIELD resource ON observation_history TYPE object FLEXIBLE;
DEFINE FIELD version_id ON observation_history TYPE uuid;
DEFINE FIELD parent_version_id ON observation_history TYPE option<uuid>;
DEFINE FIELD op_id ON observation_history TYPE uuid;
DEFINE FIELD origin_node ON observation_history TYPE uuid;
DEFINE FIELD hlc ON observation_history TYPE string;
DEFINE FIELD last_updated ON observation_history TYPE datetime;
DEFINE FIELD deleted ON observation_history TYPE bool;
DEFINE INDEX observation_history_updated ON observation_history FIELDS last_updated;

-- Generic token index: id = [type, param, system, code, resource_uuid]
DEFINE TABLE idx_token SCHEMAFULL;
DEFINE FIELD target ON idx_token TYPE record;

-- Example write transaction with optimistic check
BEGIN TRANSACTION;
  LET $id = u'0192f3a4-0000-7000-8000-000000000001';
  LET $head = (
    SELECT
      version_id,
      version_seq,
      index_keys
    FROM ONLY type::record('observation', $id)
  );
  IF $head != NONE
  AND $head.version_id != $if_match {
    THROW 'precondition-failed';
  };
  UPSERT type::record('observation', $id) CONTENT {
    resource: $resource,
    version_id: $op_id,
    version_seq: ($head.version_seq ?? 0) + 1,
    last_updated: time::now(),
    deleted: false,
    authority_node: $node,
    security_labels: [],
    profiles: [],
    index_keys: [
      type::record(
        'idx_token',
        ['Observation', 'code', 'http://loinc.org', '8480-6', $id]
      ),
    ],
    code: ['http://loinc.org|8480-6'],
    status: 'final',
  };
  CREATE type::record(
    'observation_history',
    [$id, ($head.version_seq ?? 0) + 1]
  ) CONTENT {
    resource: $resource,
    version_id: $op_id,
    parent_version_id: $head.version_id,
    op_id: $op_id,
    origin_node: $node,
    hlc: $hlc,
    last_updated: time::now(),
    deleted: false,
  };
-- (change-log and outbox rows are written here too; shape decided by the sync protocol ticket)
  FOR $k IN ($head.index_keys ?? []) {
    DELETE $k;
  };
  CREATE type::record(
    'idx_token',
    ['Observation', 'code', 'http://loinc.org', '8480-6', $id]
  ) CONTENT { target: type::record('observation', $id) };
COMMIT TRANSACTION;

-- Range scan for code search
SELECT target
FROM idx_token:['Observation', 'code', 'http://loinc.org', '8480-6', NONE]..=[
  'Observation',
  'code',
  'http://loinc.org',
  '8480-6',
  ..,
];
