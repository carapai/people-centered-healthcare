-- Central's recorded copy of the DHIS2 instance's metadata (#560; analytics.md § Reporting to
-- DHIS2; ADR 0069): the instance's data sets, their data elements and each one's category option
-- combos, as the instance exported them (JSON text). Validation checks a Report's DHIS2 mappings
-- against it. One record, `dhis2_metadata:instance`, replaced whenever a new copy is recorded.
DEFINE TABLE dhis2_metadata SCHEMAFULL;
DEFINE FIELD export ON dhis2_metadata TYPE string;
DEFINE FIELD recorded_by ON dhis2_metadata TYPE string;
DEFINE FIELD recorded_at ON dhis2_metadata TYPE datetime;
