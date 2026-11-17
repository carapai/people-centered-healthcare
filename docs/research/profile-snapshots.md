# Profile snapshots without Java or FSH import

Distilled from the research for [How are profiles authored and given snapshots, with no Java and no FSH import?](https://github.com/carapai/people-centered-healthcare/issues/518). The full findings, with the experiment, its inputs and every diff, are on the unmerged branch `research/profile-snapshots` ([profile-snapshots-findings.md](https://github.com/carapai/people-centered-healthcare/blob/research/profile-snapshots/docs/research/profile-snapshots-findings.md)). Facts were checked on 2026-11-17.

## Decision

- **Packages bring their snapshots.** The product generates none, and runs no Java (ADR 0053). A profile with only a differential is refused at Validation, as before.
- **The supported way to produce them** is SUSHI or JSON differentials, then the pinned HL7 Java validator's `snapshot` command in a CI job, then import the JSON into the Studio. `tools/fhir/build-core-package.sh` already does this for the platform core package, and published IGs already ship IG Publisher snapshots.
- **Central checks consistency at Validation.** Every differential element must appear in the snapshot with the same constraints. This catches stale or hand-edited snapshots without generating anything (#520).
- **No Studio snapshot generator for now.** If a Studio profile editor is ever wanted, it gets its own Rust generator, accepted only after a differential test against the Java tool, the same gate ADR 0053 sets for the validator.
- **Snapshots are pinned artefacts.** The Java tool's own output changes between versions, so snapshots are regenerated in the same change as any bump of the pinned validator (7.0.2 today).

## Why no generator

Each candidate ran on the R5 core profiles vitalsigns, heartrate, bp and bodyweight, and on two sliced and extension fixtures. Its output was compared with HL7's `validator_cli` 7.0.6 on the fields the tier 3 validator reads.

| Candidate | Version, licence | Result |
|---|---|---|
| `fhir-snapshot-generator` (Outburn, TypeScript) | 2.3.1, Apache-2.0; one maintainer, last release 2026-03 | Closest. Three faults change validation: applies `valueQuantity max 0` to all of `value[x]` in R5 bp, spreads a UCUM binding onto every type of `component.value[x]`, and drops `type` and `obs-3` when expanding a contentReference in a slice. It falls back silently to the shipped snapshot, and runs on Node only |
| `tlq-fhir-snapshot` (Rust) | 0.1.10, MIT; source repository gone | Fails on bp and the sliced fixture; drops reference target profiles |
| `rh-foundation` snapshot (Rust) | 0.3.0, MIT or Apache-2.0 | Follows R4 rules; rejects valid slices and sorts elements alphabetically |
| SUSHI `--snapshot` (TypeScript) | 3.21.0, Apache-2.0 | Works only from FSH, which the Studio does not import |
