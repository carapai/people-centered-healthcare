# health-client-registry

Layer: **domain** (Registries). Source Patients, golden records and their links (ADR 0027; patient-identity.md § Golden record model). Matching rules arrive with [#69](https://github.com/carapai/people-centered-healthcare/issues/69).

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
