-- Summary MeasureReports sync to central and roll up there (#555; analytics.md § Sync and Roll
-- up; ADR 0068, ADR 0075). Only summaries leave the facility.
-- On the edge:
-- - `indicator_summary.hash` is the summary's figures' hash (its computed-at time left out), and
--   `sent` whether central holds those figures, so a recompute that gives the same figures sends
--   nothing.
-- - `indicator_summary_upstream` keeps, per `[measure, period_type, period, facility]`, the hash
--   central holds, and `removed` when the summary has been forgotten here since.
-- At central:
-- - `facility_indicator_summary` keeps each facility's summary, keyed `[measure, period_type,
--   period, facility]`: a corrected one replaces it, a forgotten one is removed.
-- - `facility_indicator_sync` keeps, per facility, when its summaries last synced, so a facility
--   that has not synced shows as missing, never as zero.
DEFINE FIELD hash ON indicator_summary TYPE string DEFAULT '';
DEFINE FIELD sent ON indicator_summary TYPE bool DEFAULT false;
DEFINE INDEX indicator_summary_sent ON indicator_summary FIELDS sent;
UPDATE indicator_summary SET hash = '', sent = false;

DEFINE TABLE indicator_summary_upstream SCHEMAFULL;
DEFINE FIELD hash ON indicator_summary_upstream TYPE string;
DEFINE FIELD removed ON indicator_summary_upstream TYPE bool DEFAULT false;
DEFINE INDEX indicator_summary_upstream_removed ON indicator_summary_upstream FIELDS removed;

DEFINE TABLE facility_indicator_summary SCHEMAFULL;
DEFINE FIELD measure ON facility_indicator_summary TYPE string;
DEFINE FIELD period_type ON facility_indicator_summary TYPE string;
DEFINE FIELD period ON facility_indicator_summary TYPE string;
DEFINE FIELD facility ON facility_indicator_summary TYPE string;
DEFINE FIELD node ON facility_indicator_summary TYPE string;
DEFINE FIELD report ON facility_indicator_summary TYPE string;
DEFINE FIELD computed_at ON facility_indicator_summary TYPE datetime;
DEFINE FIELD received_at ON facility_indicator_summary TYPE datetime;
DEFINE INDEX facility_indicator_summary_period ON facility_indicator_summary FIELDS measure, period_type, period;

DEFINE TABLE facility_indicator_sync SCHEMAFULL;
DEFINE FIELD node ON facility_indicator_sync TYPE string;
DEFINE FIELD synced_at ON facility_indicator_sync TYPE datetime;
