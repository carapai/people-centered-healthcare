// Examples of each profile. They are not part of the package: the build writes them to
// examples/, where the conformance tests read them.

Instance: anc-visit-mapping
InstanceOf: ExtractionMappingLibrary
Usage: #example
* url = "https://health.go.ug/fhir/Library/anc-visit-mapping"
* version = "1.0.0"
* name = "AncVisitMapping"
* status = #active
* type = PlatformCodes#extraction-mapping
* extension[targetQuestionnaire].valueUri = "https://health.go.ug/fhir/Questionnaire/anc-visit"
* extension[targetVersions].valueString = ">=1.0.0 <2.0.0"
* contained[0] = anc-visit-bp-template
* relatedArtifact[0].type = #composed-of
* relatedArtifact[0].resourceReference = Reference(anc-visit-bp-template)

Instance: anc-visit-bp-template
InstanceOf: Observation
Usage: #inline
* extension[0].url = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/extraction-template"
* extension[0].extension[0].url = "mode"
* extension[0].extension[0].valueCode = #managed
* extension[0].extension[1].url = "retraction"
* extension[0].extension[1].valueCode = #entered-in-error
* extension[0].extension[2].url = "ownerUseCase"
* extension[0].extension[2].valueString = "clinical-data.record-observation"
* status = #final
* code = http://loinc.org#85354-9 "Blood pressure panel"

Instance: patient-matching-rules
InstanceOf: MatchingRulesLibrary
Usage: #example
* url = "https://health.go.ug/fhir/Library/patient-matching-rules"
* version = "1.0.0"
* name = "PatientMatchingRules"
* status = #active
* type = PlatformCodes#matching-rules
* content.contentType = #application/vnd.health.matching-rules+json
* content.data = "e30="

Instance: opd-visit-workflow
InstanceOf: WorkflowDefinitionLibrary
Usage: #example
* url = "https://health.go.ug/fhir/Library/opd-visit-workflow"
* version = "1.0.0"
* name = "OpdVisitWorkflow"
* status = #active
* type = PlatformCodes#workflow-definition
* content.contentType = #application/vnd.health.workflow+json
* content.data = "e30="

Instance: art-program
InstanceOf: ProgramPlanDefinition
Usage: #example
* url = "https://health.go.ug/fhir/PlanDefinition/art-program"
* version = "1.0.0"
* name = "ArtProgram"
* status = #active
* type = http://terminology.hl7.org/CodeSystem/plan-definition-type#clinical-protocol
* extension[episodeType].valueCodeableConcept.text = "HIV care"
* extension[eligibility].valueExpression.language = #text/fhirpath
* extension[eligibility].valueExpression.expression = "%patient.active"
