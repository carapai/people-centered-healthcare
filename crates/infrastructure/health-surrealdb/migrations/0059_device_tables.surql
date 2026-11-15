-- FHIR Device and DeviceAssociation (#453; ADR 0064; resource-ownership.md). Device is split by
-- category: equipment to the equipment module, patient-assigned to clinical-data.
-- DeviceAssociation, loans of Equipment to a patient, is clinical-data's.
DEFINE TABLE device SCHEMAFULL;
DEFINE FIELD json ON device TYPE string;
DEFINE FIELD version_id ON device TYPE string;
DEFINE FIELD version_seq ON device TYPE int;
DEFINE FIELD last_updated ON device TYPE datetime;
DEFINE FIELD deleted ON device TYPE bool;
DEFINE FIELD index_keys ON device TYPE array<record> DEFAULT [];
DEFINE TABLE device_history SCHEMAFULL;
DEFINE FIELD json ON device_history TYPE string;
DEFINE FIELD version_id ON device_history TYPE string;
DEFINE FIELD version_seq ON device_history TYPE int;
DEFINE FIELD last_updated ON device_history TYPE datetime;
DEFINE FIELD deleted ON device_history TYPE bool;
DEFINE INDEX device_history_last_updated ON device_history FIELDS last_updated, version_seq;
DEFINE TABLE device_association SCHEMAFULL;
DEFINE FIELD json ON device_association TYPE string;
DEFINE FIELD version_id ON device_association TYPE string;
DEFINE FIELD version_seq ON device_association TYPE int;
DEFINE FIELD last_updated ON device_association TYPE datetime;
DEFINE FIELD deleted ON device_association TYPE bool;
DEFINE FIELD index_keys ON device_association TYPE array<record> DEFAULT [];
DEFINE TABLE device_association_history SCHEMAFULL;
DEFINE FIELD json ON device_association_history TYPE string;
DEFINE FIELD version_id ON device_association_history TYPE string;
DEFINE FIELD version_seq ON device_association_history TYPE int;
DEFINE FIELD last_updated ON device_association_history TYPE datetime;
DEFINE FIELD deleted ON device_association_history TYPE bool;
DEFINE INDEX device_association_history_last_updated ON device_association_history FIELDS last_updated, version_seq;
