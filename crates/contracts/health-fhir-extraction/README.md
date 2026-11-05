# health-fhir-extraction

Layer: **contracts** (clinical-documentation). SDC template extraction of QuestionnaireResponses (questionnaire-extraction.md; ADR 0022, 0023; #93). It is a pure plan of the resources a completed response produces under an Extraction Mapping. Nothing is written here: `CompleteQuestionnaireResponse` hands each planned resource to its Owner's use case, in one transaction.

- **`Mapping::from_library`:** reads an `ExtractionMappingLibrary` (ADR 0049):
  - its target Questionnaire and version range;
  - its contained templates, each with the platform's `extraction-template` extension: the mode (`managed` or `seed`), the retraction status a managed template needs, and the Owner's use case.

  It refuses a Library of another type, a missing target, a template without an id or governing extension, and two templates with one id.
- **`extract(engine, response, mapping)`:** evaluates SDC's `templateExtractContext` and `templateExtractValue` with the FHIRPath engine (`health-fhir-validation`).
  - A context on the template gives one resource per result; one on an element repeats the element per result, and removes it when there is none.
  - A value sets a primitive (on its `_element`, with or without a placeholder) or a complex element.
  - An empty result removes the element, and an element left empty is removed.
  - Several values for an element that does not repeat are refused, as is a complex value for a primitive.
- **Evaluation:**
  - Relative paths start at the context, and `%resource` is the response.
  - Expressions are composed from the response root (`(context)[i].select(value)`), so a context never replaces `%resource`.
  - The response is the only input: there is no `now()`, query or `launchContext`.
- **Ids** are derived with `DerivationName::extraction`: `{response}|{mapping url}|{linkId path}#{repeat key}|{type}`.
  - The repeat key is the item's `id`, else its index among repeats.
  - A template without a context, or whose context is not a response item, is keyed by its own id.
  - The same mapping url in a newer version gives the same ids, and two planned resources with one id are refused.
- **What is written:**
  - decimals keep their source scale (`1.50`, ADR 0052);
  - every resource is tagged `urn:health:extraction` with its mode;
  - an Observation or DocumentReference gets `derivedFrom` the response.
- **`bundle(plan)`:** the plan as a transaction Bundle of `PUT`s, as SDC's `$extract` returns it. It serves the dry run and fixtures.
- **Not here yet:**
  - the mapping's publication gates: expressions that parse and type-check, Owner opt-in, fixtures, and `round(n)` on arithmetic;
  - `$populate`.

  `%questionnaire` is not offered: the engine binds `%resource` to the evaluation root only.

Tests: `tests/extraction.rs` covers:
- a fixture ANC mapping and response that give exactly `fixtures/anc-expected.json`;
- ids stable across amendments, removed repeats and mapping versions;
- answers that decide what is produced;
- refused outputs and malformed mappings.
