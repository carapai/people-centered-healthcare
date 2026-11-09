# health-configuration

Layer: **domain** (Terminology & Metadata). Published Definitions and their packages ([configuration.md](../../../docs/architecture/configuration.md); ADRs 0024–0026). The crate is pure: it never reads an archive, hashes or checks a signature. [`health-packages`](../../infrastructure/health-packages) does those, and hands it a package's parsed contents.

## `package`

- **`PackageJson`** is `package/package.json`. It holds:
  - the `name`, the `version` and `fhirVersions`;
  - the `dependencies`, by name;
  - the `kind`: national core, terminology, program, country configuration, or override;
  - the `platform`: `minSoftware` and `formatVersion`.
- **`PackageContents`** is what the rules check: `package.json`, the manifest Library's `depends-on` canonicals, the FHIR artefacts (path, `resourceType`, `id`, `url`, `version`) and every file's path.
- **`Version`** is an exact semver 2.0.0 version, `MAJOR.MINOR.PATCH` with an optional pre-release, ordered by semver precedence.
  - Ranges are not versions. `^`, `~`, `x`, `*`, comparators and partial versions are all refused.
  - So is build metadata, since two packages differing only in it would share one `name@version`.
- **`PackageContents::problems_for_publication`** is the publication gate's package check. It returns every `Problem`, in a stable order:
  - **Name and versions:**
    - the name must be lower-case letters, digits, `.`, `-` and `_`;
    - the package version must be exact;
    - `fhirVersions` must be exactly `["5.0.0"]`;
    - `minSoftware` must be an exact version.
  - **Pinning:**
    - every dependency must be pinned to an exact version, and a package may not depend on itself;
    - there must be a manifest (`Library-manifest.json`), and it must pin every canonical it lists as `url|version`;
    - every canonical artefact must have a version, and no `url|version` may appear twice.
  - **Layout:** files may sit only at the top level (`*.json`), under `configuration/` or under `tests/`.
- **`Canonical`** reads and prints `url|version`.

## `lifecycle`

The publication lifecycle at central (#88; configuration.md § Lifecycle; ADR 0025).

- **`Lifecycle`** is one package version on its way to publication: Draft → Validation → Simulation → Review → Approval → Published. A failed validation or simulation, or changes requested at review or approval, return it to Draft.
- **Every transition returns a `Record`:** the step, the stages before and after, who acted (`system` for the automated steps), when, and the content's fingerprint. The configuration context keeps the records and audits each one.
- **Content is pinned.** Only a draft's content may change. Submitting fixes its fingerprint, and validation, simulation, review and approval apply to that content. A return to Draft clears the review and every approval.
- **Separation of duties:**
  - whoever creates or revises the draft is an author, and only an author submits it;
  - no author reviews, approves or requests changes;
  - clinical content (national core, terminology and program packages) needs two distinct approvers, and configuration (country configuration and overrides) needs one (`approvals_required`).
- **Publication** needs every approval, and the fingerprint of what was signed must be the approved one. Nothing further happens to a published version here: activation is a separate, scoped record (#89).

### `lifecycle::semver`

The semver diff check (metadata.md § Versioning). **`check(latest, next, content)`** compares a version with the latest one published under its name. Each artefact has its own semver, so the package's bump follows from theirs:

| Change | Bump at least |
|---|---|
| An artefact's own version, or a pinned dependency, takes a patch, minor or major bump | the same |
| An artefact or a dependency is added | minor |
| An artefact or a dependency is removed, or the package's kind changes | major |
| A `configuration/` file is added or changed | minor |
| A `configuration/` file is removed | major |
| A `tests/` file is added, changed or removed | patch |

- **Refused whatever the bump:**
  - a version no newer than the latest;
  - a published `url|version` whose content changed;
  - an artefact or dependency version that went down, or is not exact.
- **Major version zero and pre-releases promise nothing:** between `0.y.z` versions a minor bump covers a major change, and between versions differing only in their pre-release any change goes.
- **`Release::of(contents, fingerprints)`** reads a release from a package's contents, leaving out the generated `package.json`, `.index.json` and manifest.
- **Each artefact's own bump** is checked against what changed in its content: `check` takes those `ContentChange`s by canonical URL, and refuses an artefact whose new version understates them (`ArtefactUnderstated`). `health-packages`' `bump` module finds them in the JSON, since this crate reads no JSON.

## `activation`

Scoped activation on nodes (configuration.md § Distribution and activation on nodes, § Rollback; #89).

- **`Activation`** is a record that package `name@version` is active in a `Scope` from `effective_at`. The scope is national, a district, a facility, or a program at a facility. Records are never changed: rollback is a new activation of an earlier version.
- **`active_set(activations, chain, now)`** merges the activations along a node's `Chain` (its district, facility and programs).
  - For each package, the most specific scope wins: program, then facility, then district, then national.
  - Within a scope, the latest-issued activation already in effect wins.
  - Central, with no district or facility, takes the national set.
- **`check(set, staged, release)`** lists what stops a set being activated (`Problem`):
  - a package not staged on the node;
  - a dependency missing, or active at a version other than the one it was published against (ADR 0024: no floating ranges at activation);
  - `minSoftware` newer than the running release;
  - a `formatVersion` the release does not read.
- **`switch(current, …)`** moves to the new set whole, or not at all.
  - It returns `Unchanged`, `Activated` with the changes by package (the caller reindexes when a changed package carries SearchParameters), or `Failed` with the problems, keeping the previous set.
- **`retained(activations, chain)`** lists every version ever activated in the node's scope. The node keeps each one while data refers to it.

## Tests

Unit tests cover exact versions and their precedence, each publication rule, and reading `package.json` as written. The lifecycle tests cover two approvers for each clinical kind and one for configuration, authors and co-authors refused as reviewers and approvers, returns to Draft clearing approvals, content pinned from submission to signing, steps out of order, and records as JSON. The activation tests cover:
- the scope chain's merge, including another district's, facility's or program's activation;
- effective times and rollback;
- the staging, dependency, software and format checks;
- a failed activation keeping the previous set;
- records as JSON. The semver tests cover each row of the table, understated and overstated versions, rewritten `url|version`s, downgrades, inexact versions, major zero and pre-releases.
