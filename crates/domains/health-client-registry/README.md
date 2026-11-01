# health-client-registry

Layer: **domain** (Registries). Source Patients, golden records and their links (ADR 0027; patient-identity.md § Golden record model). Patient matching is in `matching` (ADRs 0028, 0055).

- **`survive(field, current, incoming)`:** trust survivorship.
  - Higher trust wins: NIRA-verified, then staff-verified in person, then staff-entered, then self-reported.
  - Within a level, the most recent value wins.
  - A lower-trust value never overwrites a higher-trust one. When it disagrees, it becomes a `ReviewItem` (§62).
- **`GoldenRecord`:** the ECID, the linked source ids, each field's surviving `Claim` (value, trust, time, source), and the sources reviewers separated.
  - `link` refuses separated, inactive or already-linked sources.
  - `update` re-applies a source's newer values.
  - `unlink` adds an adjudication marker and recomputes the values from the remaining sources, so nothing of the separated source survives.
- **`plan_merge(source, target)`:** a `$merge` plan, only for two active sources of the same facility. `MergePlan::ready` refuses a plan nobody approved, so nothing is merged automatically.
- **`Ecid`:** 11 random Crockford base32 characters plus a Crockford mod-37 check character, which catches every single substitution and adjacent transposition.
  - Display is in groups of four.
  - Parsing accepts either case and the `I`/`L`/`O` aliases.
  - The randomness is supplied by the caller.
- **`matching`** (ADRs 0028 and 0055):
  - **`MatchingRules`** is the published definition, with `platform_default`: the five ADR 0055 keys, `max_block` 1,000, Double Metaphone length 6, field comparators and m/u values, null handling, and thresholds.
    - The m/u values are placeholders until the Splink calibration.
    - The thresholds are deliberately conservative: auto-link at 20 and review at 8, so same-sex twins in one household land in review.
  - **`deterministic`:** a shared verified identifier (NIRA-verified NIN, ECID, program id) is `Match`. Different verified values in the same system are `Conflict`, a hard non-match whatever the score.
  - **`index_keys` / `query_keys`:** `nin`, `phone` (last nine digits), `fam_yob_dist`, `fam_given_yob` (codes sorted, so swapped names share it) and `dist_given_yob_sex`.
    - Queries spread the birth year ±1 for `fam_given_yob`, or ±5 on every year key when the age is estimated.
    - The phonetic encoder is the `Phonetic` port.
  - **`plan`:** splits keys by stored block counts. Blocks over `max_block` are skipped (their share is a quality metric). `insufficient()` means too little to match on.
  - **`evaluate`:** Fellegi–Sunter weights with Jaro–Winkler, Damerau–Levenshtein, phonetic, exact and year comparators.
    - Term frequency only ever lowers a field's weight.
    - Null handling: conservative counts a missing value as a disagreement, moderate as half of one, greedy as nothing.
    - Agreement only inside an estimated age's ±5 band is worth half.
    - The result is a FHIR `match-grade`: certain, probable, possible or certainly-not.
- **`review`:** the registry review queue (ADR 0028).
  - **`ReviewEntry`** is a potential match (source, candidate, golden record, score) or a survivorship disagreement (golden record, field, current and incoming values).
  - **`decide`** accepts only the decisions that fit the item: same person or different people, keep current or take incoming. It needs a named reviewer, and refuses an item that is already decided.
  - **`overdue`:** pending past `AGEING` (7 days).
  - **`ReviewQueue`** is the storage port; `health-surrealdb` implements it (migration `0007_registry_review`).
- **`person`:** a Login Account's `Person` (#111; ADR 0029).
  - **`Person.link`** names a source Patient the account may act as, at an `Assurance`: `level1` from a phone code, `level3` from a NIRA NIN check or an in-person check of an ID document (`Evidence`, `Verification`).
  - **`link_existing`** links an existing Patient only on a `level3` verification: a phone account is never linked automatically. `link_self_registered` links the Patient the account's own registration created, at the account's level. `verify` raises a link's assurance and never lowers it.
  - **`access`:** no link sees nothing; below `level3`, only what the patient submitted (`PortalAccess::SelfSubmitted`); `level3` sees released records too, subject to the PDP and sensitivity labels.
