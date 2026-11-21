# health-fhir-mapping

Layer: **contracts**. FHIR R4 → R5 conversion for boundary adapters and content import (ADR 0015, 0050). R4 never enters storage: whatever reads R4 converts it here, and every conversion answers a report of what was converted and what could not be.

- **`questionnaire::read(text, version)`** reads a pasted Questionnaire as R5, or as R4 converted to R5, for the Studio form designer's *Import FHIR JSON* (#497, #853), served by central at `POST /api/studio/packages/{id}/questionnaires/import`.
  - R4's `choice` and `open-choice` items become `coding` with `answerConstraint` `optionsOnly` or `optionsOrString`; one already there is kept.
  - Unknown elements, unknown item types, `contained` resources and a `modifierExtension` are unconverted, and `blocks` says the import must not go on.
  - Read as R5, R4's choice types are refused (`looks-r4`), as are text that is not JSON and JSON that is not a Questionnaire.
- **On the JSON.** Conversion works on `serde_json::Value`, not on typed `fhir-r4` models: typed parsing drops unknown elements (ADR 0050), and the report must name them.
