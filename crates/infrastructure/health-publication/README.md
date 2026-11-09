# health-publication

Layer: **infrastructure**. Central's publication lifecycle for Published Definition packages (#88; [configuration.md](../../../docs/architecture/configuration.md) § Lifecycle; ADR 0025). The rules are [`health-configuration`](../../domains/health-configuration)'s `lifecycle` module, and the packages are [`health-packages`](../health-packages)'. Lifecycles live in `health-surrealdb`'s migration `0027_package_lifecycle`, and tarballs in the blob store ([`health-files`](../health-files)).

- **`Publications::draft(files, by)`** packs the author's files without signing them (`health_packages::pack`) and stores the tarball as a blob.
  - The tarball's sha-256 is the draft's content: what is validated, reviewed, approved and signed.
  - A second draft of the same `name@version` revises it while it is a draft, and its author becomes an author too.
  - A published `name@version` is refused: it never changes, so draft a new version.
- **`Publications::submit(id, by)`:** an author submits the draft, and the automated steps run in the same transaction:
  1. **Validation:**
     - the publication rules and the release's schemas (`health_packages::check`);
     - then, against the latest published version of the package, the semver check (`health_configuration::lifecycle::semver::check`). That covers the package's own bump, and each changed artefact's bump against its content (`health_packages::bump`).
  2. **Workflow Definitions (#96):** every Library whose type is `workflow-definition` must hold, as inline `application/vnd.health.workflow+json` content, a `health_workflow::definition::WorkflowDefinition` with its Library's url and version. It must pass `problems` against the actions given to `with_actions`, so an unknown action or an invalid parameter fails validation. With no actions given, which is the case until contexts declare theirs, any definition that executes an action fails.
  3. **Extraction Mappings (#93):** every Library whose type is `extraction-mapping` must:
     - be a usable mapping (`health_fhir_extraction::Mapping`), every expression of which evaluates;
     - name, in each template, an Owner use case that accepts its resource type (`health_clinical_documentation_app::OPT_INS`);
     - name only items of the package's Questionnaire versions in its range;
     - round the whole result of every value expression that computes (ADR 0052);
     - ship at least one `tests/` fixture of type `extraction-fixture` (`mapping` `url|version`, `response`, `expected`), each giving exactly its expected Bundle.

     A fixture naming a mapping the package does not hold fails.
  4. **Program definitions (#116):** every PlanDefinition of type `clinical-protocol` must be a usable program definition (`health_programs_app::program`). Every regimen it names that the package holds (an `ActivityDefinition`) must cite one of the program's approved protocol sources, through a `citation`, `documentation`, `justification` or `derived-from` artifact. A regimen from a dependency was checked when that dependency was published.
  5. **Simulation:** the `Sandbox` runs the `tests/` cases.
     - **`WorkflowSandbox`** (#101) runs every `tests/` JSON file of type `workflow-scenario` against the package's definition it names, on the same interpreter, with stubbed actions and guards (`health_workflow::simulation`).
     - **Failures:** every scenario must pass, and one naming a definition the package does not hold fails.
     - **Other cases:** indicator cases are not run yet.
     - **`NoSandbox`** runs none, for tests of the other steps.

  A failure returns the draft to Draft, with every problem in its record.
- **People's steps:** `review`, `approve` and `request_changes`, with the domain's separation of duties:
  - no author reviews or approves;
  - clinical kinds need two distinct approvers, and configuration kinds one;
  - changes requested clear the review and every approval.
- **`Publications::publish(id, by, signer)`** needs every approval.
  - It signs the stored tarball with the publication-signing key (`PublicationSigner`, held in the HSM or KMS; `LocalSigner` until then), after checking it again.
  - The signed sha-256 must be the approved one.
  - It records the `PublishedPackage`: the digest, the signature, who published it, the authors, the reviewer and the approvers. It is recorded once; a second publication of the same id fails.
- **Reading:**
  - `lifecycle(id)` returns the lifecycle and its records, oldest first;
  - `open()` lists every lifecycle not yet published;
  - `package(id)` returns a published package and its tarball.
- **Audit:** every transition is a record and an AuditEvent (`package-lifecycle`), in the transaction that makes it.
- **Concurrency:** each change is a compare-and-set on the lifecycle's revision. Of two people acting at once, one gets `Error::Conflict` rather than overwriting the other.

- **`Publications::activate(package, version, scope, effective_at, by, reason)`** (#89; ADR 0025) issues an `ActivationRecord`: the published version, its scope (national, district, facility or program), when it takes effect (now by default), and the tarball's sha-256 and signature so nodes can fetch and verify it.
  - A version that is not published is refused.
  - The record is kept once (`package_activation`) and logged for every node below, in the packages lane. It is audited (`package-activation`).
  - Rollback is another activation, of an earlier version. `activations()` lists them all, oldest first.

## Tests

`tests/publication.rs` runs over SurrealDB in memory, a blob store in a temporary directory, and a real root with a publication-signing certificate:
- **A clinical package:**
  - an author is refused as reviewer and as approver;
  - one approval is not enough;
  - after two, it publishes, and the tarball central serves verifies under the root as a node would check it;
  - its records list every step;
  - drafting or publishing it again is refused.
- **Validation:**
  - an unpinned dependency sends the draft back with the problem;
  - a revision by another person makes them an author, who may then not approve;
  - nothing changes after submission;
  - changes requested clear the approval;
  - configuration publishes with one approval.
- **Semver against the latest published version:**
  - a removed item under a minor bump is refused, with the change named;
  - the same change as a major version passes;
  - a form changed under its old version is refused.
- **A stale revision** is refused by the store.
- **Unknown packages** are not found.

`tests/workflows.rs` runs a program package with a Workflow Definition against one declared action:
- a valid definition reaches Review;
- an unknown action, a parameter of the wrong kind, and a missing parameter each send the draft back, with the problem prefixed by its file;
- content of another type, a definition of another version, and content that is not a definition are refused.
