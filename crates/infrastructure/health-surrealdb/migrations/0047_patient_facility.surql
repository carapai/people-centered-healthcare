-- Which facilities a patient has records at (#81; synchronization.md § Pull and Sync Scope: an
-- edge receives golden-record Patient updates and links only for patients who have records at
-- the facility): keyed `[patient id, facility]`, written with every facility-tagged record that
-- references the patient, and when a facility sends a Patient it registered.
DEFINE TABLE patient_facility SCHEMAFULL;
DEFINE FIELD at ON patient_facility TYPE datetime;
