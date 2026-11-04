// The engine-native Published Definitions (ADR 0049): three profiled Libraries and the program
// PlanDefinition. Every one is a canonical artefact with its own semver.

RuleSet: PublishedDefinition
* url 1..1
* version 1..1
* name 1..1

Invariant: hp-template-1
Description: "Every contained template says how it governs its target."
Expression: "contained.all(extension('https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/extraction-template').exists())"
Severity: #error

Profile: ExtractionMappingLibrary
Parent: Library
Id: ExtractionMappingLibrary
Title: "Extraction Mapping Library"
Description: "An Extraction Mapping (ADR 0022, 0049): the SDC templates, contained, that turn a form's answers into clinical records, with the Questionnaire and versions it applies to."
* ^status = #active
* ^experimental = false
* insert PublishedDefinition
* obeys hp-template-1
* type = PlatformCodes#extraction-mapping
* extension contains
    ExtractionTargetQuestionnaire named targetQuestionnaire 1..1 and
    ExtractionTargetVersions named targetVersions 1..1
* contained 1..*
* contained ^short = "The SDC templates, with templateExtractContext and templateExtractValue"
* contained ^comment = "Each template is referenced from relatedArtifact as composed-of, as dom-3 requires of a contained resource."
* relatedArtifact 1..*

Profile: MatchingRulesLibrary
Parent: Library
Id: MatchingRulesLibrary
Title: "Matching Rules Library"
Description: "Matching Rules (ADR 0028, 0049): the rules that find a patient's candidate matches, as JSON content."
* ^status = #active
* ^experimental = false
* insert PublishedDefinition
* type = PlatformCodes#matching-rules
* content 1..1
* content.contentType 1..1
* content.contentType = #application/vnd.health.matching-rules+json

Profile: WorkflowDefinitionLibrary
Parent: Library
Id: WorkflowDefinitionLibrary
Title: "Workflow Definition Library"
Description: "A Workflow Definition (ADR 0042, 0049): an engine-native statechart as JSON content. A PlanDefinition it was derived from is cited as derived-from."
* ^status = #active
* ^experimental = false
* insert PublishedDefinition
* type = PlatformCodes#workflow-definition
* content 1..1
* content.contentType 1..1
* content.contentType = #application/vnd.health.workflow+json

Profile: ProgramPlanDefinition
Parent: PlanDefinition
Id: ProgramPlanDefinition
Title: "Program PlanDefinition"
Description: "A program definition (ADR 0049): a clinical protocol whose enrolment opens an EpisodeOfCare, linking its Workflow Definitions, Questionnaires, Extraction Mappings, Measures, ActivityDefinitions and protocol sources."
* ^status = #active
* ^experimental = false
* insert PublishedDefinition
* type 1..1
* type = http://terminology.hl7.org/CodeSystem/plan-definition-type#clinical-protocol
* extension contains
    ProgramEpisodeType named episodeType 1..1 and
    ProgramEligibility named eligibility 0..1
