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

## Tests

Unit tests cover exact versions and their precedence, each publication rule, and reading `package.json` as written.
