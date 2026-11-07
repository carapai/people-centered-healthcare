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

// Extensions the platform's modules write on the resources they own: a completed form's mapping
// (#93), a program's episode (#116), a care plan's author and a stock product's units and names.

Extension: ExtractionMapping
Id: extraction-mapping
Title: "Extraction Mapping used"
Description: "The Extraction Mapping, with its version (`url|version`), that a completed QuestionnaireResponse was extracted with. Later amendments of the response reuse it (questionnaire-extraction.md)."
Context: QuestionnaireResponse
* ^status = #active
* ^experimental = false
* value[x] only canonical
* value[x] 1..1

Extension: EpisodeProgram
Id: episode-program
Title: "Episode program"
Description: "The canonical URL of the program (a ProgramPlanDefinition) an EpisodeOfCare is an enrolment in."
Context: EpisodeOfCare
* ^status = #active
* ^experimental = false
* value[x] only canonical
* value[x] 1..1

Extension: EpisodeEnrolledBy
Id: episode-enrolled-by
Title: "Episode enrolled by"
Description: "Who enrolled the patient in the program, as the id of their account."
Context: EpisodeOfCare
* ^status = #active
* ^experimental = false
* value[x] only string
* value[x] 1..1

Extension: EpisodeTransferredFrom
Id: episode-transferred-from
Title: "Episode transferred from"
Description: "The EpisodeOfCare at another facility that this one continues, after a transfer."
Context: EpisodeOfCare
* ^status = #active
* ^experimental = false
* value[x] only Reference(EpisodeOfCare)
* value[x] 1..1

Extension: EpisodeTransferredTo
Id: episode-transferred-to
Title: "Episode transferred to"
Description: "The code of the facility the patient was transferred to; the episode is finished here and continued there."
Context: EpisodeOfCare
* ^status = #active
* ^experimental = false
* value[x] only string
* value[x] 1..1

Extension: CarePlanAuthor
Id: care-plan-author
Title: "Care plan author"
Description: "Who wrote a CarePlan, as the id of their account."
Context: CarePlan
* ^status = #active
* ^experimental = false
* value[x] only string
* value[x] 1..1

Extension: ProductBaseUnit
Id: product-base-unit
Title: "Product base unit"
Description: "The unit a stock product is counted in, e.g. tablet. Stock quantities and pack sizes are in this unit."
Context: Medication
* ^status = #active
* ^experimental = false
* value[x] only string
* value[x] 1..1

Extension: ProductPackSize
Id: product-pack-size
Title: "Product pack size"
Description: "A pack size a stock product comes in, in base units; one extension per size."
Context: Medication
* ^status = #active
* ^experimental = false
* value[x] only unsignedInt
* value[x] 1..1

Extension: ProductSynonym
Id: product-synonym
Title: "Product synonym"
Description: "Another name a stock product is known by, used to match prescriptions to it; one extension per name."
Context: Medication
* ^status = #active
* ^experimental = false
* value[x] only string
* value[x] 1..1
