# Patient matching is layered (deterministic, blocking, Fellegi–Sunter) and driven by published Matching Rules

`Patient/$match` runs four steps in `client-registry`:
1. a **deterministic** short-circuit on a NIRA-verified NIN, an existing ECID or a verified program id;
2. **blocking** on several strict keys, such as phonetic family name plus birth year, NIN, phone, and district plus given-name initial;
3. **Fellegi–Sunter** scoring with term-frequency adjustment and explicit null handling;
4. classification into auto-link, potential match or new identity.

Two different verified identifiers in the same system are a hard non-match, whatever the score.

**Rules as data.** Fields (as FHIRPath), comparators, m/u values, blocking keys and thresholds form a versioned **Matching Rules** Published Definition. It goes through the normal package lifecycle, so a country tunes matching without code changes. The m/u values are calibrated offline with Splink EM on de-identified data.

**Launch thresholds.** v1 auto-links only on a deterministic verified-identifier match, or a score above a threshold set to measured precision ≥ 99.9%. Everything in the potential band goes to registry review. Thresholds are relaxed only by publishing new rules.

**Candidate lookup.** Candidates come from SurrealDB array-keyed blocking index tables written in the same transaction as the Patient. A scale spike at about 45M people validates this; the fallback is a tantivy sidecar at central.

**Edges.** At registration, the edge asks central first, with a 3 s timeout. Offline, it runs the same engine against its own local scope only.

We chose this for three reasons:
- About 25% of people lack a NIN, so matching must work on demographics.
- Ugandan names and shared phones need frequency-aware weights.
- The OpenHIE registry spec requires matching rules that non-programmers can configure.

## Considered Options

- **Deterministic rules only** (OpenCR's default). Rejected: they are too brittle for spelling variants and name-order swaps.
- **An ML classifier.** Rejected for v1: it is opaque to reviewers and needs labelled data we do not have.
- **OpenSearch as the candidate index.** Deferred unless the spike fails.

## Consequences

- Registry review queues need staffing, and an ageing alert fires after 7 days.
- Reviewer decisions feed precision and recall reporting.
