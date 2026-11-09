# Reference packages

Published Definition packages the platform ships as references (configuration.md; ADR 0024). Each directory is a package, in the layout `health_packages` packs. `health-publication`'s `tests/reference_packages.rs` checks that each one passes validation and its `tests/` scenarios.

| Package | What | Issue |
|---|---|---|
| `lab/` (`ug.moh.lab`) | The lab Workflow Definition (`Library/lab-test`) and its workflow scenarios | #109 |
| `opd/` (`ug.moh.opd`) | OPD triage: the triage Questionnaire, the triage categories, and the Extraction Mapping that records vital signs, with its fixtures | #103 |

**Sources:** a Workflow Definition is inline, base64 content in its Library. Its readable source is in `sources/`, and the test checks the Library holds exactly that source. Edit the source, then regenerate the Library's `content[0].data` from it.

## `lab/`: the laboratory test

A test is a workflow instance about its ServiceRequest. Laboratory delivers each step to it (`health-fhir-server`'s `lab`):

| State | Worklist | Leaves on |
|---|---|---|
| `collect` | `lab-collect` | `laboratory.collected`, or `laboratory.closed` (cancelled) |
| `result` | `lab-result` | `laboratory.resulted`, or `laboratory.closed` (specimen rejected) |
| `verify` | `lab-verify` | `laboratory.verified`, or `laboratory.verified-critical` |
| `review` | `clinician-results` | `laboratory.reviewed` |
| `review-critical` | `clinician-results`, priority `stat` | `laboratory.reviewed` |

The clinician's worklist gets a test only once its results are verified, and a critical result arrives at `stat` priority.

## `opd/`: outpatient triage

- **`Questionnaire/opd-triage`:** a nurse records vital signs and a triage category.
  - The vital signs are temperature, pulse, respiratory rate, SpO2, weight, height, and blood pressure (systolic and diastolic).
  - The categories are `CodeSystem/triage-category`: `emergency`, `urgent` and `routine`. The category is required.
- **`Library/opd-triage-mapping`:** each answered vital sign becomes a managed Observation, owned by `clinical-data.record-observation`.
  - Each carries category `vital-signs`, its LOINC code and a UCUM quantity. It names the response's patient and encounter, takes its time from `authored`, and is `derivedFrom` the response.
  - Blood pressure is one panel (`85354-9`) with systolic (`8480-6`) and diastolic (`8462-4`) components. It is recorded only when both are answered.
  - A vital sign later left blank is retracted as `entered-in-error`.

| Vital sign | LOINC | Unit |
|---|---|---|
| Temperature | 8310-5 | `Cel` |
| Pulse | 8867-4 | `/min` |
| Respiratory rate | 9279-1 | `/min` |
| Oxygen saturation | 59408-5 | `%` |
| Weight | 29463-7 | `kg` |
| Height | 8302-2 | `cm` |
| Blood pressure | 85354-9 (8480-6, 8462-4) | `mm[Hg]` |

- **Fixtures** (`tests/`):
  - `triage-emergency` has every vital sign, giving seven Observations.
  - `triage-partial` has temperature and pulse, and a systolic pressure with no diastolic, giving two Observations and no panel.
- **The triage category** is not extracted. The triage screen sends it to the visit's workflow as its completing event, and the OPD Workflow Definition (#105) records it as the visit's `urgency`. An urgent or emergency visit then never waits at a Financial Gate.
