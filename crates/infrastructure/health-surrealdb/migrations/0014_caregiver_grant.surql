-- Caregiver grants (#113; patient-identity.md § Caregiver delegation): one record per grant,
-- found by its patient or its caregiver; the FHIR RelatedPerson and Consent are written beside
-- it. Owned by consent.
DEFINE TABLE caregiver_grant SCHEMAFULL;
-- The caregiver's phone Login Account.
DEFINE FIELD caregiver ON caregiver_grant TYPE string;
-- The dependent's source Patient.
DEFINE FIELD patient ON caregiver_grant TYPE string;
-- The grant, as JSON written by consent.
DEFINE FIELD data ON caregiver_grant TYPE string;
DEFINE INDEX caregiver_grant_patient ON caregiver_grant FIELDS patient;
DEFINE INDEX caregiver_grant_caregiver ON caregiver_grant FIELDS caregiver;
