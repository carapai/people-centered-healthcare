-- Each facility's HMIS reporting settings at central (#560; ADR 0069; facility-configuration.md):
-- its HMIS org unit ID and the DHIS2 data sets (forms) reported through the platform for it, kept
-- as JSON text (`health_facility_registry::FacilityReporting`), one record per facility keyed by
-- its code, with who set it and when. Only central sets them.
DEFINE TABLE facility_reporting SCHEMAFULL;
DEFINE FIELD setting ON facility_reporting TYPE string;
DEFINE FIELD set_by ON facility_reporting TYPE string;
DEFINE FIELD set_at ON facility_reporting TYPE datetime;
