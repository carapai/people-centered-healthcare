# Drug interactions come from a curated national Interaction List, not a licensed knowledge base

Decided in [Where do drug interaction checks get their knowledge, and how do they act at order-sign?](https://github.com/carapai/people-centered-healthcare/issues/786), after the research [What drug interaction knowledge can the platform use in Uganda, and on what terms?](https://github.com/carapai/people-centered-healthcare/issues/785) ([drug-interactions.md](https://github.com/carapai/people-centered-healthcare/blob/research/drug-interactions/docs/research/drug-interactions.md), branch `research/drug-interactions`). It settles the drug-drug interaction check that [ADR 0074](0074-decision-support-is-fhirpath-decision-tables-translated-from-who-content-with-no-cql-runtime.md) left waiting for a knowledge base.

The research found:

- Commercial knowledge bases (First Databank, Medi-Span, Lexidrug, Micromedex, Stockley's) publish no prices, no low-income-country terms and no terms for copying data to offline servers.
- Open sets cannot be used: DDInter and DrugBank are non-commercial, DDInter's files leave out rifampicin with ART, the Liverpool checkers forbid reproduction, and NLM's RxNav interaction API ended in January 2024.
- Small curated sources exist: the ONC high-priority list (15 drug-class pairs, plus 33 pairs that should not interrupt) and the HIV medicine interaction table in the Uganda Clinical Guidelines 2023. ATC codes cannot be redistributed commercially.
- No interaction source is recommended by NDA or MoH; UgandaEMR and ClinicMaster check nothing in practice; 18.7% of ART patients in a Kampala cohort had clinically significant interactions.
- Clinicians override 49–96% of drug alerts; few interruptive alerts work best.

**The decision:**

- **Interaction List.** Interaction knowledge is a curated national **Interaction List**, a configuration package published and activated like others and synced to edges and desktops. Each pair is a FHIR R5 `ClinicalUseDefinition` of type `interaction`: two interactants, effect, severity, management text, source and last review date. It is seeded from the ONC high-priority pairs and the Uganda Clinical Guidelines ART table, stated as facts with their source and management text written in the platform's own words.
- **Interactants** are ingredients or named drug classes; class membership (for example rifamycins: rifampicin, rifabutin) lives in the same package. No ATC codes.
- **One built-in Decision Support Rule** reads the list, so governance edits data, not rules. It runs offline on the node holding the record, at order-sign and when a pharmacist records a dispense.
- **What is compared.** A new order or dispense is checked against the other orders being signed, the patient's active prescriptions, and dispenses within a look-back (30 days by default), including those from other facilities in the shared record.
- **Severity.** Contraindicated gives critical Advice, which needs an override with a reason, recorded as a `DetectedIssue`. Major gives warning Advice beside the order, without interrupting. Moderate shows as info on the chart. Pairs on the ONC non-interruptive list are never critical. Emergency care is never stopped.
- **Advice** names both medicines (and the facility of an outside one), the effect, the management and the source. Its actions are: cancel the new order, stop or change the existing prescription, or override.
- **Overrides persist** for that pair and patient while both medicines stay active; later orders show the pair as info, citing the override.
- **Unchecked Products.** A medicine Product without ingredients cannot be checked and does not block; a governance report lists such Products.
- **Governance.** A national clinical governance role (the MoH medicines and therapeutics function, or the deployment's equivalent) maintains the list. Facilities cannot remove or downgrade pairs; the look-back is an Override Point. A monthly report gives alert and override counts per pair.
- **Deferred.** A connector to a licensed knowledge base, behind the same rule, once a buyer and terms exist; duplicate-therapy and dose-range checks.

## Considered options

- **A licensed knowledge base:** broad and maintained, but no published price or offline redistribution terms, and every edge and desktop would need a copy.
- **DDInter or DrugBank:** large and coded, but non-commercial licences, and DDInter's downloads miss the ART–TB interactions that matter most in Uganda.
- **Pairs as decision-table rows:** works with ADR 0074 as is, but every pair edit would be a rule edit with its own tests; `ClinicalUseDefinition` keeps the knowledge as data.
- **Interrupting on major interactions:** catches more, but the override rates in the research show interruptive alerts are ignored when frequent.
