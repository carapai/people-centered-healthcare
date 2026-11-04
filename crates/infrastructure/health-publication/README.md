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
  2. **Simulation:** the `Sandbox` runs the `tests/` cases. **`NoSandbox` runs none:** the simulation sandbox is #101, and until then simulation passes every package.

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

## Not yet

- **`central-server` commands** for each step (#88 part 3b).
- **Activation,** the scoped and dated record that puts a published version to use, is #89.

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
