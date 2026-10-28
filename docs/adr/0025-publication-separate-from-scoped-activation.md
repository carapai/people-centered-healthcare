# Publishing a package never activates it; activation is a scoped, dated record, and rollback is re-activation

The §45 lifecycle (Draft → Validation → Simulation → Review → Approval → Publication → Activation → Supersession) runs at central.

- **Drafts** never leave central.
- **Approvals.** Review and approval must be done by people other than the author. Clinical content (metadata, terminology, program packages) needs two distinct approvers; configuration needs one.
- **Publication** produces an immutable, signed version.
- **Activation** is a separate global record: package version, scope (national, district, facility or program) and effective date. It syncs down in the metadata lane.
- **On a node,** activation:
  1. stages and verifies the package;
  2. checks its dependencies and software compatibility;
  3. switches the active set atomically at the effective time.

  If any check fails, the node keeps its previous set and reports why.
- **Supersession** is a newer activation in the same scope. **Rollback** is a new activation pointing at an earlier version. Nothing is mutated or deleted.

**Pinning.** Every record pins the definition versions it was created under:
- the Questionnaire `url|version`;
- the profile version;
- the Extraction Mapping version;
- the workflow-definition version;
- `Coding.version` for local versioned CodeSystems.

Old records are therefore always interpreted with the definitions they were made under.

We chose this so that the same immutable version can go live at different times in different places (pilot facilities first, then a district). It also makes rollback safe, since it removes nothing, and it makes "which definitions governed this record?" answerable forever.

## Considered Options

- **Publication implies national activation.** Rejected: there would be no pilots, no staged roll-out and no per-facility timing.
- **Rollback by un-publishing or deleting a version.** Rejected: records already written under it would lose their interpretation.
- **Records referencing definitions without a version.** Rejected: re-rendering an old record under a newer form would change its meaning.

## Consequences

- Each node's active set is the merge of activations along its scope chain (national → district → facility).
- A SearchParameter change in an activated package starts a reindex.
