# Default blocking uses five narrow keys with an hourly stop-list, and blocking recall is proven before launch

**Default keys.** The platform's default Matching Rules (ADR 0028) use five blocking keys. `dist_init` (district + given initial) is dropped.

| Key | Value | Why |
|---|---|---|
| `nin` | National ID number | Exact identifier |
| `phone` | Normalized phone | Shared within households, so it has a low weight (ADR 0028) |
| `fam_yob_dist` | Phonetic family name + birth year + district | The main demographic key |
| `fam_given_yob` | The phonetic family and given codes, **sorted**, + birth year; no district | Finds people registered in another district, and does not depend on name order |
| `dist_given_yob_sex` | District + phonetic given name + birth year + sex | Finds people whose surname is missing or changed |

**Rules:**
- **Birth-year tolerance.** Queries expand birth year by ±1 for `fam_given_yob` only, or by ±5 when the birth date is marked as estimated. The other keys use the exact year.
- **Phonetic algorithm.** It is a field of the Matching Rules, defaulting to Double Metaphone primary codes of length 6. Calibration may replace it with a Uganda-tuned variant in a new rules version.
- **Stop-list.**
  - **Counts.** Block counts are recomputed hourly into a `block_count` table: at central, and on each edge for its local scope. They are never updated in the write transaction.
  - **Skipping.** A key whose block exceeds the rules' `max_block` (default 1,000) is skipped at query time.
  - **Metric.** The share of keys skipped is reported as a matching-quality metric.
- **Too little to match on.** If every key of a query is absent or skipped, `$match` returns no candidates, with an OperationOutcome saying there is too little identifying data. The "Did you mean" step then falls back to a manual registry search by name, birth year, and village or parish.
- **Blocking recall** is the share of true pairs that share at least one block, and it is proven twice:
  - **In CI,** a synthetic corruption benchmark (typos, swapped names, ±1–5-year ages, missing NIN, shared phones) must keep blocking recall at **≥ 99%** and candidate sets under 2,000.
  - **Before launch,** blocking recall is measured on de-identified real data, using NIN-verified duplicate pairs. This runs with the Splink calibration under the DPPA basis in the map's fog.

We chose this because of the matching-scale spike [Can SurrealDB blocking indexes serve patient matching at 45M-person scale?](https://github.com/carapai/people-centered-healthcare/issues/37). At 45M, 99% of queries on the original keys exceed 2,000 candidates, mostly because of `dist_init`. Narrow keys plus a stop-list keep every query under 2,000, at a projected p95 of under 10 ms.
- **Why recompute counts hourly.** Range-scan `LIMIT` does not bound cost, so stored counts are the only real guard. Updating them on every write would cause conflicts on hot surname keys.
- **Why the recall gate.** A pair that never shares a block can never be matched, so blocking recall caps everything downstream.

## Considered Options

- **The original four keys:** fail the candidate target. Rejected.
- **Block counts updated in the write transaction:** write conflicts on common keys. Rejected.
- **Scanning oversized blocks with a `LIMIT`:** does not bound cost, and returns an arbitrary subset. Rejected.
- **A tantivy sidecar:** not needed for exact-key blocking, because the bottleneck is block size. Rejected for now.

## Consequences

- `idx_block` rows per patient rise from about 3.3 to about 4.5, and the spike measured about 110 µs of single-threaded write cost per 3.3 rows.
- Matching quality reports gain the skipped-key share and blocking recall.
- The registration UX needs a manual registry search for the "too little to match on" case.
