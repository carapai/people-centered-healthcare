# Reference packages

Published Definition packages the platform ships as references (configuration.md; ADR 0024). Each directory is a package, in the layout `health_packages` packs. `health-publication`'s `tests/reference_packages.rs` checks that each one passes validation and its `tests/` scenarios.

| Package | What | Issue |
|---|---|---|
| `anc/` (`ug.moh.anc`) | The ANC program (`PlanDefinition/anc`, whose episode type is the test patients' `anc` EpisodeOfCare type, so the HMIS line lists list their enrollments), the ANC indicators (*ANC 1st visits*, *ANC 1st visit before 12 weeks*, the median gestational age at the first visit, *ANC visits*, *ANC 1st visits with the next visit booked*), the ANC visit types, and the test patients the indicator editor's *Try* evaluates them over | #548, #549, #550, #552 |
| `core/` (`ug.moh.core`) | MoH's national core: the Age Band Sets HMIS reports disaggregate by (*MoH ANC age bands*, *MoH outpatient age bands*), and the regions and districts places sit in, with their boundaries | #551, #869, #604 |
| `hmis/` (`ug.moh.hmis`) | A Reporting package: the HMIS 105 antenatal care Report (`configuration/reports/hmis-105-anc.json`, monthly, facility to national: ANC attendance by MoH's ANC age bands with row totals and % change, sent to DHIS2 with each cell mapped (#560) to illustrative IDs, and the timing of the 1st visit) and the facility's ANC monitoring (facility only: the median gestational age at the 1st visit, a line list of ANC clients with their latest gestational age, and one of the 1st visits counted); and the standard *ANC overview* Dashboard (`configuration/dashboards/anc-overview.json`: ANC 1st visits by area and by age band over the last 6 months, the share before 12 weeks over 12, and this quarter's total, each over the viewer's own area) | #556, #603 |
| `lab/` (`ug.moh.lab`) | The lab Workflow Definition (`Library/lab-test`) and its workflow scenarios | #109 |
| `population/` (`ug.population.2026`) | A Population package (ADR 0071): total population for the sample districts (illustrative figures, not UBOS's) and expected pregnancies derived as 5% of it, at `configuration/population.json` | #553 |
| `opd/` (`ug.moh.opd`) | The outpatient visit: the OPD Workflow Definition (`Library/opd-visit`) and its scenarios, triage (the Questionnaire, the triage categories, and the Extraction Mapping that records vital signs, with its fixtures), and the triage route, whose guards name a decision table (`Library/opd-triage-route`) | #103, #105, #846 |

**DHIS2 metadata:** `dhis2/ehmis-metadata.json` is a recorded copy of a DHIS2 instance's `dataSets` export (illustrative IDs, not eHMIS's), which `ug.moh.hmis`'s mappings are checked against; record it at central with `central-server package dhis2-metadata`. Replace both with MoH's eHMIS IDs once its metadata is exported (#560).

**Sources:** a Workflow Definition, and an Age Band Set's bands, are inline, base64 content in their Library. The readable source is in `sources/`, and the test checks the Library holds exactly that source. Edit the source, then regenerate the Library's `content[0].data` from it.

## `anc/`: ANC indicators

- **`Measure/anc-first-visits`** counts women whose first antenatal care visit was in the period, each once (`cohort`, basis `Encounter`, counting people).
  - **Period date:** `Encounter.actualPeriod.start`, when the visit started.
  - **Criteria** (`initial-population`): the visit's type is `first` in `CodeSystem/anc-visit-type`, and `%patient.gender` is `female`.
  - **Disaggregated** (#552) by MoH's ANC age bands, `https://health.go.ug/fhir/Library/age-bands-anc|1.0.0` in `core/`: in March 2026, 15–19 Akello (17), 20–24 Nakato (23), 25+ Namubiru (31) and Nansubuga (26), none 10–14 or not recorded.
- **`Measure/anc-first-visit-before-12-weeks`** (`proportion`, a percentage): the denominator is the same first visits; the numerator those where the gestational age Observation (LOINC `18185-9`) recorded at the visit, read through `%records`, is under 12 weeks.
- **`Measure/anc-gestational-age-at-first-visit`** (`continuous-variable`, `median`): the gestational age at the first visit, each woman's first value.
- **`Measure/anc-visits`** (`cohort`, counting visits): every ANC visit by a woman, first or follow-up.
- **`Measure/anc-first-visit-next-visit-booked`** (`proportion`, #550): the denominator is the women in *ANC 1st visits*' numerator (`'https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0' in %indicators.numerator`); the numerator those whose booked next visit (an `Appointment`, read through `%records`) is within 4 weeks after the period's end.
- **Test patients** (`tests/patient-*.json`, `type` `test-patient`), one Patient and their Encounters each, each ANC visit with its EpisodeOfCare (the enrollment) and, where measured, its gestational age:

| Patient | Visits | March 2026 |
|---|---|---|
| Akello | first on 11 March | counted |
| Nakato | first on 4 March, follow-up on 28 March | counted once |
| Namubiru | first at 22:30 on 31 March, entered on 3 April | counted (care counts when it happened) |
| Nansubuga | first recorded twice, 25 and 27 March | counted once |
| Auma | first on 7 January, follow-ups in February and March | not counted |
| Atim | first on 27 February | not counted |
| Nabirye | first on 8 April | not counted (counted in April) |
| Okello | an outpatient visit on 2 March | not counted |
| Opio | a man whose visit was coded as a first ANC visit | not counted |
| Kisakye | a first visit planned, not started | not counted (no period date) |
| Achan | a first visit in November 2025, a follow-up on 10 February, then a new pregnancy's first visit on 16 June | not counted |

So March 2026 is 4, April 1 and 2026Q1 6.

Gestational ages at first visits: Akello 16, Nakato 10 (14 at her follow-up), Namubiru 9, Nansubuga 8 then 11 at the duplicate, Auma 12, Atim 13, Nabirye 11. So in March, *ANC 1st visit before 12 weeks* is 3 of 4 (75%) and 2026Q1 3 of 6 (50%); the median gestational age at the first visit is 9.5 (10.5 taking the last value) and 2026Q1 11. *ANC visits* in March is 7 visits by 5 women; January to June, 13 visits by 8 women in 9 pregnancies.

Next visits booked (`Appointment`, status `booked`): Akello 8 April, Nakato 28 April, Auma 15 April, Namubiru 6 May, Nansubuga 30 March (her 10 April one is cancelled). So in March, *ANC 1st visits with the next visit booked* is 2 of 4 (Akello and Nakato, 50%): Namubiru's is 5 weeks after the period's end and Nansubuga's before it.

## `core/`: MoH's Age Band Sets (#551; ADR 0071)

Each is a `Library` of type `age-band-set` (`national-core`, so two approvers), its bands in whole years, both ends included:

| Set | Age taken at | Bands |
|---|---|---|
| `Library/age-bands-anc`, *MoH ANC age bands* | the end of the period | 10–14, 15–19, 20–24, 25+ |
| `Library/age-bands-outpatient`, *MoH outpatient age bands* | the date of the visit | 0, 1–4, 5–9, 10–19, 20+ |

The local e2e backend publishes the package, so the Studio's *Age band sets* section shows both.

### Regions and districts (#869; ADR 0094)

`Library/administrative-areas` (type `administrative-areas`, content `application/vnd.health.administrative-areas+json`) lists a sample of the country's regions and districts: Central (Kampala, Wakiso, Mukono), Eastern (Jinja, Mbale), Northern (Gulu) and Western (Mbarara), coded `region-…` and `district-…`. The facility registry's NHFR import links each facility to its district by that code and refuses a district not listed here.

`Library/administrative-boundaries` (type `administrative-boundaries`, content `application/geo+json`; #604) holds their boundaries as a GeoJSON `FeatureCollection`, one simplified polygon per region and district, each named by its code, which maps shade by an indicator's threshold bands. They are rough sample outlines, a few points each, not survey geometry. The source is `sources/administrative-boundaries.geojson`, a boundary file naming areas as UBOS does (`Central Region`, `KAMPALA`); `central-server boundaries core sources/administrative-boundaries.geojson` imports it, matching each feature to an area by name, checking it against the hierarchy (a district inside its region) and writing the Library, and central-server's test checks the shipped Library is exactly that import. Validation refuses boundaries without the areas in the same package, of an area not listed, given twice, not a closed polygon, or a district outside its region.

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
- **The triage category** is not extracted. It is the triage task's completing event, `opd.triaged-<category>`, and the visit's workflow records it as `urgency` (below).

### `Library/opd-visit`: the visit (#105; workflow-engine.md § Reference shapes)

One Encounter-scoped instance, whose `urgency` data field is the triage category.

| State | Task (worklist) | Leaves on |
|---|---|---|
| `reception` | none | `opd.arrived` → triage; `opd.arrived-emergency` → consultation (urgency `emergency`) |
| `triage` | nurse (`opd-triage`), form `opd-triage` | `opd.triaged-routine` / `-urgent` → payment; `opd.triaged-emergency` → consultation, **bypassing the gate** |
| `payment` | Financial Gate `before-consultation`; the cashier (`cashier`) while it blocks | `gate.pass` or `gate.defer` → consultation |
| `consultation` | clinician (`opd-consult`) | `opd.orders-placed` → orders; `opd.seen` → disposition |
| `orders` (parallel) | none | `lab`: `laboratory.reviewed` or `opd.no-lab`; `pharmacy`: `pharmacy.dispensed` or `opd.no-pharmacy`; when both are back → review |
| `review` | clinician (`opd-review`, urgent) | `opd.orders-placed` → orders again; `opd.seen` → disposition |
| `disposition` | clerk (`opd-disposition`) | `opd.discharged`, `opd.admitted` or `opd.referred` (final) |

- **The gate** reads `urgency`, so urgent and emergency care are never blocked (ADR 0033). An emergency never reaches it.
- **Node requirements:** the definition uses no guards or typed actions, so a node runs it as written. Lab and pharmacy report back as events rather than invoked sub-workflows.

Scenarios in `tests/`:
- `opd-visit-routine`: paid, seen, both orders back, seen again, discharged.
- `opd-visit-emergency-triage` and `opd-visit-emergency-arrival`: the gate is set to block, and the visit never reaches it.
- `opd-visit-unpaid`: blocked at the cashier until paid.
- `opd-visit-urgent`: the gate defers, and there are no lab or pharmacy orders.

### `Library/opd-triage-route`: where triage sends a patient (#846; ADR 0093)

Each exit from `triage` (nurse, `opd-triage`, form `opd-triage`) on `opd.triaged` is guarded by the outcome of the decision table `triage-priority`, in document order:

| Rule | When | Outcome |
|---|---|---|
| Danger signs | `%triage.danger = true` | `emergency` |
| Triaged emergency | `%triage.category = 'emergency'` | `emergency` |
| Triaged urgent | `%triage.category = 'urgent'` | `urgent` |
| (default) | | `routine` |

`emergency` goes to `emergency` (clinician, `opd-emergency`, stat), `urgent` straight to `consultation`, and anything else to `payment` (the cashier) first. The inputs read the data requirement `triage` (an `Observation`'s `category` and `danger`). A node does not run it until FHIRPath guards are wired; the Studio shows it and plays it in the sandbox.

Tests in `tests/`: `opd-triage-route-emergency`, `-urgent` and `-routine` stub the table's outcome (`tables`), and `triage-priority-danger-signs` (a `decision-table-test`) checks that danger signs outrank an urgent category.
