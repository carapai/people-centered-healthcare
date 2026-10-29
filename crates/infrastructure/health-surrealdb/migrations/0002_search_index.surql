-- Search index tables (persistence.md § Search index tables) and the reindex job ledger. Each
-- index record id is an array ordered for range scans, ending with the resource id; the `rt` and
-- `param` fields repeat its first two elements so the secondary indexes can lead with them.
-- Every head table gains `index_keys`, the entries its current version wrote, so a write
-- deletes exactly those. Generated with src/ownership/generated.rs; never edit after merge.

-- [type, param, system, code, resource_id]
DEFINE TABLE idx_token SCHEMAFULL;
DEFINE FIELD rt ON idx_token TYPE string;
DEFINE FIELD param ON idx_token TYPE string;
DEFINE FIELD code ON idx_token TYPE string;
DEFINE INDEX idx_token_code ON idx_token FIELDS rt, param, code;

-- [type, param, folded_value, resource_id]
DEFINE TABLE idx_string SCHEMAFULL;
DEFINE FIELD rt ON idx_string TYPE string;
DEFINE FIELD param ON idx_string TYPE string;
DEFINE FIELD exact ON idx_string TYPE string;

-- [type, param, start, end, resource_id], plus `end` for overlap and ends-before queries
DEFINE TABLE idx_date SCHEMAFULL;
DEFINE FIELD rt ON idx_date TYPE string;
DEFINE FIELD param ON idx_date TYPE string;
DEFINE FIELD end ON idx_date TYPE datetime;
DEFINE INDEX idx_date_end ON idx_date FIELDS rt, param, end;

-- [type, param, target_type, target_id, resource_id], plus `target_id` for id-only references
DEFINE TABLE idx_reference SCHEMAFULL;
DEFINE FIELD rt ON idx_reference TYPE string;
DEFINE FIELD param ON idx_reference TYPE string;
DEFINE FIELD target_id ON idx_reference TYPE string;
DEFINE INDEX idx_reference_target_id ON idx_reference FIELDS rt, param, target_id;

-- [type, param, system, code, value, resource_id], plus value-first indexes for searches that
-- leave the unit system or the unit open
DEFINE TABLE idx_quantity SCHEMAFULL;
DEFINE FIELD rt ON idx_quantity TYPE string;
DEFINE FIELD param ON idx_quantity TYPE string;
DEFINE FIELD code ON idx_quantity TYPE string;
DEFINE FIELD value ON idx_quantity TYPE decimal;
DEFINE INDEX idx_quantity_value ON idx_quantity FIELDS rt, param, value;
DEFINE INDEX idx_quantity_code_value ON idx_quantity FIELDS rt, param, code, value;

-- [type, param, value, resource_id]
DEFINE TABLE idx_number SCHEMAFULL;
DEFINE FIELD rt ON idx_number TYPE string;
DEFINE FIELD param ON idx_number TYPE string;

-- [type, param, value, resource_id]
DEFINE TABLE idx_uri SCHEMAFULL;
DEFINE FIELD rt ON idx_uri TYPE string;
DEFINE FIELD param ON idx_uri TYPE string;

-- [type, param, component values..., resource_id]
DEFINE TABLE idx_composite SCHEMAFULL;
DEFINE FIELD rt ON idx_composite TYPE string;
DEFINE FIELD param ON idx_composite TYPE string;

-- [type, param, resource_id]: a supported parameter with no value, for :missing and :not
DEFINE TABLE idx_missing SCHEMAFULL;
DEFINE FIELD rt ON idx_missing TYPE string;
DEFINE FIELD param ON idx_missing TYPE string;

-- One row per reindex job; `after` is the last resource id processed, so a restart resumes.
DEFINE TABLE reindex_job SCHEMAFULL;
DEFINE FIELD rt ON reindex_job TYPE string;
DEFINE FIELD params ON reindex_job TYPE array<string>;
DEFINE FIELD after ON reindex_job TYPE option<string>;
DEFINE FIELD processed ON reindex_job TYPE int;
DEFINE FIELD done ON reindex_job TYPE bool;

DEFINE FIELD index_keys ON account TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON activity_definition TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON allergy_intolerance TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON appointment TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON appointment_response TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON audit_event TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON capability_statement TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON care_plan TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON care_team TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON charge_item TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON charge_item_definition TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON claim TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON claim_response TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON clinical_impression TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON code_system TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON composition TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON concept_map TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON condition TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON consent TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON contract TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON coverage TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON coverage_eligibility_request TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON coverage_eligibility_response TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON detected_issue TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON device_definition TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON device_request TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON diagnostic_report TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON document_reference TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON encounter TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON endpoint TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON episode_of_care TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON explanation_of_benefit TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON family_member_history TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON flag TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON goal TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON group TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON guidance_response TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON healthcare_service TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON imaging_study TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON immunization TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON immunization_evaluation TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON immunization_recommendation TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON implementation_guide TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON insurance_plan TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON inventory_item TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON inventory_report TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON invoice TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON library TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON location TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON measure TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON measure_report TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON medication TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON medication_administration TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON medication_dispense TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON medication_knowledge TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON medication_request TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON medication_statement TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON naming_system TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON nutrition_order TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON observation TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON organization TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON patient TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON payment_notice TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON payment_reconciliation TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON person TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON plan_definition TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON practitioner TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON practitioner_role TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON procedure TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON questionnaire TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON questionnaire_response TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON related_person TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON request_orchestration TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON risk_assessment TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON schedule TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON search_parameter TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON service_request TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON slot TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON specimen TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON structure_definition TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON supply_delivery TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON supply_request TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON task TYPE array<record> DEFAULT [];
DEFINE FIELD index_keys ON value_set TYPE array<record> DEFAULT [];
