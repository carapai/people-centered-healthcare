# Integrations

The catalogue of external connections and how failed inbound messages are handled. Decided in [What is the v1 integration catalogue, and how are failed inbound messages reviewed and replayed?](https://github.com/carapai/people-centered-healthcare/issues/813) ([ADR 0092](../adr/0092-integrations-are-a-catalogue-of-configured-connections-with-one-inbox-for-failed-messages.md)); facts on branch `research/integration-catalogue`. Validation of inbound payloads, the Inbound Dead Letter and quarantine are in [fhir-r5-model.md](fhir-r5-model.md) (ADR 0051).

## Integrations

Each external connection is a configured **Integration**: kind, node, status (off, on, failing), credentials as a secret on that node, retention, and last success and failure. A central catalogue lists them; one failing longer than a set time raises an alert.

| Integration | Runs on | Direction | v1 |
|---|---|---|---|
| DHIS2 (HMIS reports, 033b) | Central | Out; baselines in | Yes (analytics.md, surveillance.md) |
| SMS aggregator | Central | Out | Yes (communications.md) |
| Payment gateways | Central | Both | Yes (billing-and-payments.md) |
| Orthanc | Facility Edge Node | Both | Optional (imaging.md) |
| Analyser links | Facility Edge Node | In; host query out | Yes |
| CPHL viral load through UgISL | Central | Requests out, results in | Off until credentials |
| UgandaEMR migration | Central | In, once | Yes |
| Payer claim exports | Central | Out (files) | Yes (pricing-and-insurance.md) |

## Analyser links

- One port on the Facility Edge Node, with adapters for ASTM E1394 or HL7 v2 over TCP (GeneXpert first) and for export files. The desktop never connects to analysers; a facility without an edge types results.
- GeneXpert's host query is answered with the pending test for the specimen's barcode.
- Results match the request by accession barcode and arrive as **preliminary**; a laboratory professional verifies them before release.
- A result matching nothing becomes an **Unmatched Result**: the laboratory matches it to a request or discards it with a reason. It never attaches to a patient on its own.
- Not in v1: serial (RS-232) links and vendor middleware.

## CPHL viral load

- A central adapter sends viral load requests referred to CPHL through UgISL (OpenHIM) and polls for results, which become `DiagnosticReport`s on the request.
- UgISL's request JSON is FHIR-like but not valid FHIR, so the adapter maps it itself rather than putting it through FHIR validation.
- Off until MoH or METS issue credentials. Fallback: import CPHL's dashboard CSV. EID waits for its request route.

## FHIR R4 peers

- A one-time UgandaEMR migration import reads FHIR R4, converts it to R5 (`health-fhir-mapping`) and maps codes through ConceptMaps. No ongoing sync and no R4 API.
- National HIE adapters (Client Registry, Shared Health Record, data warehouse, EAC cross-border) wait until MoH confirms production and the FHIR version and profiles.

## Payers

No insurer API in v1: each payer has a claim export from a configurable template, and Smart visit codes and preauthorization numbers are recorded by hand. An API adapter goes behind the payer port when a payer offers one.

## Integration Inbox

- At central, and on the edge for edge-side Integrations.
- Lists Inbound Dead Letter and quarantine entries by Integration, age and error; shows the payload, with every view audited.
- Actions: **replay as-is** (validated again from tier 1) or **discard with a reason**. Payloads are never edited: fix the mapping or the source, then replay.
- Only the integration administrator role uses it.
- Transient network failures retry automatically and never reach it.
