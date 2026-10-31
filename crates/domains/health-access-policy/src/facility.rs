//! How a facility-scope record names its facility (ADR 0057): a `meta.tag` with system
//! [`FACILITY_TAG_SYSTEM`] and the facility code, on every record of a type in
//! [`FACILITY_SCOPE_TYPES`] (write-authority.md § Data authority matrix).

/// The `meta.tag` system whose code is a facility-scope record's facility.
pub const FACILITY_TAG_SYSTEM: &str = "urn:health:facility";

/// The resource types whose records are facility-scope, sorted: the clinical records, documents
/// and program records, workflow instances, and charges and invoices of the data authority
/// matrix. Facility-internal Locations, devices and stock follow with their own contexts.
pub const FACILITY_SCOPE_TYPES: &[&str] = &[
    "AllergyIntolerance",
    "CarePlan",
    "CareTeam",
    "ChargeItem",
    "ClinicalImpression",
    "Communication",
    "Composition",
    "Condition",
    "DeviceRequest",
    "DiagnosticReport",
    "DocumentReference",
    "Encounter",
    "EpisodeOfCare",
    "FamilyMemberHistory",
    "Flag",
    "Goal",
    "ImagingStudy",
    "Immunization",
    "Invoice",
    "MedicationAdministration",
    "MedicationDispense",
    "MedicationRequest",
    "MedicationStatement",
    "NutritionIntake",
    "NutritionOrder",
    "Observation",
    "Procedure",
    "QuestionnaireResponse",
    "RequestOrchestration",
    "RiskAssessment",
    "ServiceRequest",
    "Specimen",
    "Task",
];

/// Whether records of `resource_type` are facility-scope.
pub fn is_facility_scope(resource_type: &str) -> bool {
    FACILITY_SCOPE_TYPES.binary_search(&resource_type).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_answers_by_type() {
        assert!(
            FACILITY_SCOPE_TYPES.windows(2).all(|w| w[0] < w[1]),
            "sorted"
        );
        assert!(is_facility_scope("Observation"));
        assert!(!is_facility_scope("Patient"));
        assert!(!is_facility_scope("Organization"));
    }
}
