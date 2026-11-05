-- Laboratory tests (#109). `lab_test` is owned by laboratory: each test from its order to its
-- review, as JSON, keyed by its ServiceRequest's id. The FHIR Specimen, Observations and
-- DiagnosticReport are its projections.
DEFINE TABLE lab_test SCHEMAFULL;
DEFINE FIELD data ON lab_test TYPE string;
DEFINE FIELD patient ON lab_test TYPE string;
DEFINE FIELD stage ON lab_test TYPE string;
DEFINE INDEX lab_test_patient ON lab_test FIELDS patient;
