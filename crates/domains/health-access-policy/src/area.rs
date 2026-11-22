//! What a session at an area may not read (ADR 0094): a session acting for a district, region or
//! the national place reads summaries, Dashboards, Reports and the registries, never
//! patient-level records by virtue of the area. Patient-level access still needs a role at the
//! facility (ADR 0056).

use crate::facility::is_facility_scope;

/// The resource types of the FHIR R5 Patient compartment, sorted: the records about one person,
/// which an area session never reads or searches. Facility-scope types
/// ([`crate::FACILITY_SCOPE_TYPES`]) are patient-level too.
pub const PATIENT_COMPARTMENT_TYPES: &[&str] = &[
    "Account",
    "AdverseEvent",
    "AllergyIntolerance",
    "Appointment",
    "AppointmentResponse",
    "AuditEvent",
    "Basic",
    "BodyStructure",
    "CarePlan",
    "CareTeam",
    "ChargeItem",
    "Claim",
    "ClaimResponse",
    "ClinicalImpression",
    "Communication",
    "CommunicationRequest",
    "Composition",
    "Condition",
    "Consent",
    "Contract",
    "Coverage",
    "CoverageEligibilityRequest",
    "CoverageEligibilityResponse",
    "DetectedIssue",
    "DeviceAssociation",
    "DeviceRequest",
    "DeviceUsage",
    "DiagnosticReport",
    "DocumentReference",
    "Encounter",
    "EncounterHistory",
    "EnrollmentRequest",
    "EpisodeOfCare",
    "ExplanationOfBenefit",
    "FamilyMemberHistory",
    "Flag",
    "Goal",
    "GuidanceResponse",
    "ImagingSelection",
    "ImagingStudy",
    "Immunization",
    "ImmunizationEvaluation",
    "ImmunizationRecommendation",
    "Invoice",
    "List",
    "MeasureReport",
    "MedicationAdministration",
    "MedicationDispense",
    "MedicationRequest",
    "MedicationStatement",
    "MolecularSequence",
    "NutritionIntake",
    "NutritionOrder",
    "Observation",
    "Patient",
    "Person",
    "Procedure",
    "Provenance",
    "QuestionnaireResponse",
    "RelatedPerson",
    "RequestOrchestration",
    "ResearchSubject",
    "RiskAssessment",
    "Schedule",
    "ServiceRequest",
    "Specimen",
    "SupplyDelivery",
    "SupplyRequest",
    "Task",
    "VisionPrescription",
];

/// Whether records of `resource_type` are patient-level: in the Patient compartment or
/// facility-scope.
pub fn is_patient_level(resource_type: &str) -> bool {
    PATIENT_COMPARTMENT_TYPES
        .binary_search(&resource_type)
        .is_ok()
        || is_facility_scope(resource_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_covers_the_facility_scope() {
        assert!(
            PATIENT_COMPARTMENT_TYPES.windows(2).all(|w| w[0] < w[1]),
            "sorted"
        );
        assert!(
            crate::FACILITY_SCOPE_TYPES
                .iter()
                .all(|t| is_patient_level(t))
        );
        assert!(is_patient_level("Patient"));
        assert!(is_patient_level("Device"));
        assert!(!is_patient_level("Organization"));
        assert!(!is_patient_level("Location"));
        assert!(!is_patient_level("Questionnaire"));
    }
}
