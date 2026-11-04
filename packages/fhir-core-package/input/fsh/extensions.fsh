// Extensions of the engine-native Published Definitions (ADR 0049).

Extension: ExtractionTargetQuestionnaire
Id: extraction-target-questionnaire
Title: "Extraction target Questionnaire"
Description: "The canonical URL of the Questionnaire an Extraction Mapping applies to, without a version: the versions are in extraction-target-versions."
Context: Library
* ^status = #active
* ^experimental = false
* value[x] only uri
* value[x] 1..1

Extension: ExtractionTargetVersions
Id: extraction-target-versions
Title: "Extraction target versions"
Description: "The semver range of the target Questionnaire's versions an Extraction Mapping applies to, e.g. `>=1.2.0 <2.0.0`."
Context: Library
* ^status = #active
* ^experimental = false
* value[x] only string
* value[x] 1..1

Extension: ExtractionTemplate
Id: extraction-template
Title: "Extraction template"
Description: "How a contained template of an Extraction Mapping governs what it produces: its mode, the status a managed target takes when it disappears, and the Owner's use case that accepts it."
Context: Resource
* ^status = #active
* ^experimental = false
* extension contains
    mode 1..1 and
    retraction 0..1 and
    ownerUseCase 1..1
* extension[mode].value[x] only code
* extension[mode].value[x] 1..1
* extension[mode].valueCode from TemplateModes (required)
* extension[retraction] ^short = "The status a managed target takes when it disappears from the output, e.g. entered-in-error"
* extension[retraction].value[x] only code
* extension[retraction].value[x] 1..1
* extension[ownerUseCase] ^short = "The Owner's use case that accepts the target, checked against its opt-in list at publication"
* extension[ownerUseCase].value[x] only string
* extension[ownerUseCase].value[x] 1..1

Extension: ProgramEpisodeType
Id: program-episode-type
Title: "Program episode type"
Description: "The EpisodeOfCare type a program's enrolment opens."
Context: PlanDefinition
* ^status = #active
* ^experimental = false
* value[x] only CodeableConcept
* value[x] 1..1

Extension: ProgramEligibility
Id: program-eligibility
Title: "Program eligibility"
Description: "Who may enrol in a program: a FHIRPath expression over the patient, true when eligible."
Context: PlanDefinition
* ^status = #active
* ^experimental = false
* value[x] only Expression
* value[x] 1..1
* valueExpression.language = #text/fhirpath
* valueExpression.expression 1..1
