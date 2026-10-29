# Research: patient identity and matching

A condensed version of the research ticket [What patient matching and MPI approaches suit a national-scale, offline-first registry?](https://github.com/carapai/people-centered-healthcare/issues/10). The full findings, with citations, are on the unmerged branch `research/patient-identity`. Decisions: [docs/architecture/patient-identity.md](../architecture/patient-identity.md) (ADRs 0027–0029).

## Matching

- **Fellegi–Sunter** (as described by Splink):
  - per-field m/u probabilities give additive log₂ Bayes factors, which convert to a match probability and are thresholded;
  - term-frequency adjustment boosts rare values and discounts common ones;
  - blocking is "the single most important determinant" of speed, and 1M records give about 500 billion unblocked pairs;
  - Splink is Python/SQL and learns m/u without labels (EM).
- **OpenHIE toolkit:** "automated methods without thorough manual review increase the risk of inappropriately identifying separate individuals".
- **OpenHIE Client Registry spec:**
  - required: create, update, query by identifier, query by demographics, link/dedup, and an accept/reject review UI;
  - recommended: configurable rules and blocking, parent/child and multiple-birth support.
- **OpenCR:**
  - rules use FHIRPath per field with comparators (jaro-winkler, damerau-levenshtein, exact);
  - deterministic and probabilistic (m/u) templates exist, with `autoMatchThreshold` and `potentialMatchThreshold`;
  - null handling is conservative, moderate or greedy;
  - the source gets `refer` → golden and the golden record gets `seealso` → source;
  - human-adjudication tags keep manual decisions from being undone;
  - the prototype was informed by Uganda MoH and CPHL, but whether it is in national use today is unverified.

## FHIR R5

- **`Patient/$match`** (Trial Use): returns a scored searchset with `match-grade`; the algorithm is implementation-defined.
- **`Patient/$merge`** (Trial Use, maturity 0): the source gets `replaced-by` and `active=false`. Unmerge is not defined.
- **Link types:**
  - `replaced-by`: no longer to be used;
  - `replaces`: the current record;
  - `refer`: valid but not the main source;
  - `seealso`: valid; points to another record.
- **Person** is a linkage resource that "SHALL NOT be referenced" by clinical resources. `Person.link.assurance` runs from level1 to level4 (NIST-based). A link can itself disclose sensitive information.
- **RelatedPerson** covers guardians and caregivers as actors. `Patient.contact` cannot be a reference target.
- **IHE PMIR** keeps a golden Patient Master Identity fed by ITI-93, reusing PDQm and PIXm. Unmerge is out of scope.

## Uganda

- **NIN coverage:** NIRA reports 35.58M registered (about 75%, September 2026). Birth registration of under-ones was about 30% (2023).
- **NIRA Third Party Interface:**
  - modes: consent-based YES/NO on 4 parameters, or a consent-based full-detail return;
  - onboarding: MoU, VPN and onboarding steps;
  - free for government;
  - legal basis: ROPA s.67.
- **MoH HIE guidelines (2023)** define a Client Registry holding ECIDs and mandate FHIR. They name no NIN or matching approach.
- **UgandaEMR duplicates** are attributed to the lack of a national identifier. The figure of about 5.8% among PLHIV is unverified.

## Offline identity elsewhere

- **DHIS2 Android** reserves batches of generated ids, which can run out or expire, and has a potential-duplicate review resource.
- **OpenSRP** downloads OpenMRS IDGen batches.

## Risks carried into the design

- **False links are a patient-safety risk**, hence conservative thresholds and reversible links.
- **Matching must work without a NIN**, because many people have none.
- **Name variability and shared phones** reduce discriminating power.
- **Calibration data access** falls under the DPPA.
- **Scale.** SurrealDB handles exact-key blocking well, but the base key set is too broad at 45M (spike below). The default keys and stop-list are in ADR 0055.

## Matching-scale spike (2026-10-29)

The spike was [Can SurrealDB blocking indexes serve patient matching at 45M-person scale?](https://github.com/carapai/people-centered-healthcare/issues/37). It was a reduced run on a laptop (macOS x86_64 i9, 32 GB), **not central hardware**, using SurrealDB 3.3.5 with RocksDB `sync=every`. The client was HTTP `/sql` from Rust.

**Synthetic population:**
- **Households:** mean size 5. A shared surname with probability 0.6 and a shared phone, so 70% of people have a phone.
- **Surnames:** Zipf-Mandelbrot over 4 regional pools of 1,250 surnames each.
- **Districts:** 146, Zipf-skewed.
- **Ages:** median age about 17, and 30% of adults have their age rounded to a multiple of 5.
- **NIN:** 59%, with 0.1% duplicated.
- **Shared phones:** 3,000 health-worker phones.
- **Phonetic code:** Double Metaphone, length 6.
- **No typos were injected,** so recall was not measured.

**Candidate sets at the full 45M** (counted without a database, 20k sampled queries):

| Key set | Union per query: median / p95 / max | Queries over 2,000 |
|---|---|---|
| Base: `fam_yob` + `nin` + `phone` + `dist_init` | 32,490 / 261,683 / 476,551 | 99.1% |
| `fam_yob` alone | 1,230 / 14,992 / 32,906 | 39.8% |
| `fam_yob_dist` + `nin` + `phone` | 31 / 770 / 5,079 | 1.07% |
| `fam_yob_dist` + `nin` + `phone` + `dist_given_yob_sex` | 152 / 1,836 / 9,404 | 4.4% |
| The same, skipping any block over 1,000 | 130 / 854 / 1,887 | **0%** (3% of keys skipped) |

- **`dist_init`** fails the target by itself: its median block is 28k people per query.
- **Adding sex or region** barely helps. The common phonetic codes are already regional, and Double Metaphone merges Okello and Akello, and Wasswa, Waiswa and Asio.

**SurrealDB at 1M, 5M and 10M** (warm, base keys, one query across all keys):

| N | p50 / p95 / p99 | Rows scanned p95 | Cost per row (wall) | Load (patients/s, 8 threads) | Size with `idx_block` |
|---|---|---|---|---|---|
| 1M | 3.7 / 25.5 / 43.6 ms | 5,848 | 4.3 µs | 27,960 | 144 MiB |
| 5M | 17.2 / 114 / 175 ms | 25,219 | 4.5 µs | 17,316 | 941 MiB |
| 10M | 27.5 / 192 / 295 ms | 53,487 | 3.7 µs | 22,234 | 1.65 GiB |

- **Scaling.** Range-scan cost follows the number of rows returned, not the table size, from 1M to 10M while the data is cached.
- **Write cost of the index.** `idx_block` (3.3 rows per patient) costs about 3.6× the write time per patient (about +110 µs single-threaded) and about 2.9× the disk.
- **`LIMIT` is not a guard.** On a range scan it does not bound cost proportionally (20–45 µs per returned row), so a stop-list must use block counts kept outside the scan.

**Projection to 45M**, assuming flat per-row cost and an index of about 7–8 GB held in memory:
- **Base keys:** p95 about 1 s.
- **Refined key sets:** p95 7–36 ms.
- **Tantivy.** The exact-key bottleneck is block size, which tantivy would not change, so the tantivy fallback is not needed for exact-key candidate generation. Fuzzy name search was not tested.

**Not run:**
- a 45M load;
- central hardware;
- concurrent or mixed load;
- a true cold cache;
- recall of the refined keys;
- tantivy.

