// The platform's own codes (ADR 0049 § Consequences): the kinds of engine-native Library, the
// modes of an extraction template (ADR 0023), and the Provenance activity of an extraction run.

CodeSystem: PlatformCodes
Id: platform
Title: "Health platform codes"
Description: "Codes the platform's engines define: the kinds of engine-native Published Definition carried as a Library, the modes of an extraction template, and the Provenance activity of an extraction run."
* ^status = #active
* ^experimental = false
* ^caseSensitive = true
* ^content = #complete
* #extraction-mapping "Extraction Mapping" "A Library of SDC templates stating which answers on a form become which clinical records."
* #matching-rules "Matching Rules" "A Library of the rules that find candidate matches for a patient."
* #workflow-definition "Workflow Definition" "A Library holding an engine-native statechart: the states, steps, timers and decisions of a process."
* #managed "Managed" "An extracted resource the form keeps governing: re-extraction versions it, and it is retracted when its answers disappear."
* #seed "Seed" "An extracted resource created once from the form, which then lives a life of its own."
* #extraction "Extraction" "A Provenance activity: the resources were extracted from a completed QuestionnaireResponse by an Extraction Mapping."

ValueSet: TemplateModes
Id: template-modes
Title: "Extraction template modes"
Description: "How an extraction template governs the resource it produces (ADR 0023)."
* ^status = #active
* ^experimental = false
* PlatformCodes#managed
* PlatformCodes#seed
