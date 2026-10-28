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
- **Scale.** Blocking at about 45M people on SurrealDB is unverified; see [Can SurrealDB blocking indexes serve patient matching at 45M-person scale?](https://github.com/carapai/people-centered-healthcare/issues/37).
