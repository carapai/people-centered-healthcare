# Integrations are a catalogue of configured connections, with one inbox for failed messages

Decided in [What is the v1 integration catalogue, and how are failed inbound messages reviewed and replayed?](https://github.com/carapai/people-centered-healthcare/issues/813), after the research [Which lab analysers, national lab systems, FHIR R4 peers and insurer APIs must the platform integrate with in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/812) ([integration-catalogue.md](https://github.com/carapai/people-centered-healthcare/blob/research/integration-catalogue/docs/research/integration-catalogue.md), branch `research/integration-catalogue`). It extends [ADR 0051](0051-raw-json-structural-check-before-typed-parsing.md), which keeps rejected payloads in the Inbound Dead Letter, and settles the analyser links that `desktop-client.md` left out of v1.

The research found:

- MoH's standard analysers include Sysmex, Roche cobas, BD FACSPresto, Abbott m2000 and GeneXpert. GeneXpert sends ASTM E1394 or HL7 v2.5 in ASTM E1381 framing over TCP and answers host queries; many others only export result files.
- CPHL viral load runs through UgISL, an OpenHIM instance: UgandaEMR sends FHIR-like JSON that is not valid FHIR and polls for results returned as R4 Bundles. CPHL's dashboard offers a CSV download. No public API was found for ALIS or the sample-tracking system.
- MoH's HIE guidelines (2023) require FHIR but name no version or implementation guide; whether the Client Registry and Shared Health Record are in production is unconfirmed. UgandaEMR speaks FHIR R4.
- No Ugandan insurer publishes a provider API; Smart Applications uses its own client, visit codes and biometrics. The national scheme is not law.
- OpenHIM and Mirth review failed messages by status and rerun them, linking reruns to the original.

**The decision:**

- **Integration.** Each external connection is a configured **Integration**: kind, the node it runs on, status (off, on, failing), credentials as a secret on that node, retention, and last success and failure. A central catalogue lists every Integration, and one failing longer than a set time raises an alert. The v1 catalogue: DHIS2 (HMIS and 033b), the SMS aggregator, payment gateways, Orthanc, analyser links, CPHL through UgISL, the UgandaEMR migration import, and payer claim exports.
- **Analyser links** run on the Facility Edge Node behind one port: ASTM E1394 or HL7 v2 over TCP (GeneXpert first, with host query answered from the pending test for the barcode), and an export-file adapter. The desktop never connects to analysers; a facility without an edge types results. Serial links and vendor middleware are deferred.
- **Results from analysers are preliminary.** A laboratory professional verifies them before release, as with typed results. They match the request by the specimen's accession barcode; a result that matches nothing becomes an **Unmatched Result**, which the laboratory matches to a request or discards with a reason. It never attaches to a patient on its own.
- **CPHL viral load.** A central adapter sends viral load requests referred to CPHL through UgISL and polls for results, which become `DiagnosticReport`s on the request. The adapter maps UgISL's FHIR-like JSON itself rather than treating it as FHIR. It stays off until MoH or METS issue credentials. Importing CPHL's dashboard CSV is the fallback. EID waits for its request route.
- **FHIR R4 peers.** A one-time migration import from UgandaEMR reads FHIR R4, converts to R5 and maps codes through ConceptMaps. There is no ongoing UgandaEMR sync and no R4 API. Adapters for the national HIE (Client Registry, Shared Health Record, data warehouse, EAC cross-border) wait until MoH confirms production and the FHIR version and profiles.
- **Payers.** No insurer API adapter in v1. Each payer has a claim export from a configurable template; Smart visit codes and preauthorization numbers are recorded by hand. An API adapter is added behind the payer port when a payer offers one.
- **Integration Inbox.** At central, and on the edge for edge-side Integrations, an **Integration Inbox** lists Inbound Dead Letter and quarantine entries by Integration, age and error, and shows the payload with every view audited. Actions: replay as-is or discard with a reason. Payloads are never edited; the mapping or source is fixed, then the entry is replayed. Only the integration administrator role uses it. Transient network failures retry automatically and never reach it.

## Considered options

- **Typed results only:** nothing to build, but transcription errors on high-volume haematology and GeneXpert results, and GeneXpert's host query is wasted.
- **Analyser links on the desktop:** the bench PC is next to the analyser, but desktops come and go, and two could send the same result; the edge is always on.
- **Editing payloads in the inbox:** fixes one message fast, but changes clinical data outside the clinical record and hides the broken mapping that will fail again.
- **An R4 API for peers:** UgandaEMR and the HIE speak R4, but no peer is confirmed to read from us, and a second API version doubles what must be tested.
- **Integrating Smart's client:** covers the largest intermediary, but it has no public API and runs its own software at the facility.
