# Terminology & Metadata owns every Published Definition

Every definitional artefact (CodeSystem, ValueSet, ConceptMap, StructureDefinition, Questionnaire, PlanDefinition, ActivityDefinition, Library, Measure, program definitions, workflow definitions, extraction mappings, form layouts) is owned by Terminology & Metadata and goes through one versioned publication lifecycle. Programs, Workflow and Analytics consume pinned published versions and own only runtime records (EpisodeOfCare and outcomes, workflow instances, indicator values). We chose one owner so that versioning, approval, packaging and historical interpretation work the same way for every artefact, as recommended by the CRMI-style packaging research.

## Consequences

- The Program Designer and Workflow Studio author drafts inside Terminology & Metadata; their runtime contexts never edit definitions.
