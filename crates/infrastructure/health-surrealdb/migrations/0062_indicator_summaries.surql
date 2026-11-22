-- Indicators computed on the node (#554; analytics.md § Compute and Store; ADR 0068), owned by
-- the analytics module. Membership never leaves the node; summaries are what dashboards read
-- and what syncs to central.
-- - `indicator_member` keeps one patient's membership in one indicator and period, keyed
--   `[measure, period_type, period, patient]`: the patient's records as they came out, as JSON.
-- - `indicator_summary` keeps one indicator's summary MeasureReport for one period and
--   facility, keyed `[measure, period_type, period, facility]`, as JSON.
-- - `indicator_state:node` records where they stand on the change log.
DEFINE TABLE indicator_member SCHEMAFULL;
DEFINE FIELD patient ON indicator_member TYPE string;
DEFINE FIELD data ON indicator_member TYPE string;
DEFINE INDEX indicator_member_patient ON indicator_member FIELDS patient;

DEFINE TABLE indicator_summary SCHEMAFULL;
DEFINE FIELD measure ON indicator_summary TYPE string;
DEFINE FIELD period_type ON indicator_summary TYPE string;
DEFINE FIELD period ON indicator_summary TYPE string;
DEFINE FIELD facility ON indicator_summary TYPE string;
DEFINE FIELD start ON indicator_summary TYPE string;
DEFINE FIELD end ON indicator_summary TYPE string;
DEFINE FIELD report ON indicator_summary TYPE string;
DEFINE FIELD computed_at ON indicator_summary TYPE datetime;
DEFINE INDEX indicator_summary_period ON indicator_summary FIELDS period_type, period;

DEFINE TABLE indicator_state SCHEMAFULL;
DEFINE FIELD epoch ON indicator_state TYPE string;
DEFINE FIELD seq ON indicator_state TYPE int;
