-- Encounters and patient-flow queues (#102). `encounter_state` is owned by encounters: each
-- Encounter as the module keeps it, as JSON; the FHIR Encounter is its projection. `queue_entry`
-- is owned by patient-flow: each patient waiting at a service point, keyed by its derived id,
-- so a replayed encounter event queues no one twice.
DEFINE TABLE encounter_state SCHEMAFULL;
DEFINE FIELD data ON encounter_state TYPE string;
DEFINE TABLE queue_entry SCHEMAFULL;
DEFINE FIELD data ON queue_entry TYPE string;
DEFINE FIELD encounter ON queue_entry TYPE string;
DEFINE FIELD service_point ON queue_entry TYPE string;
DEFINE FIELD open ON queue_entry TYPE bool;
DEFINE INDEX queue_entry_service_point ON queue_entry FIELDS service_point, open;
DEFINE INDEX queue_entry_encounter ON queue_entry FIELDS encounter;
