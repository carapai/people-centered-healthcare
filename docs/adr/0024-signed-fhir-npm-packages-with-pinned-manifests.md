# Published Definitions ship as signed, immutable FHIR NPM packages with pinned manifests

Every Published Definition is distributed in a FHIR NPM package:
- a tarball with `package.json`, semver, dependencies and `fhirVersions: ["5.0.0"]`;
- a CRMI-style manifest Library that pins every dependency `url|version`, including ValueSet expansions;
- a `configuration/` folder of JSON-Schema-validated, non-FHIR configuration;
- a `tests/` folder of fixtures;
- a detached signature from a publication-signing intermediate under the platform root.

**Kinds.** There are five package kinds: national core, terminology, program, country configuration, and district or facility override.

**Pinning.** Authors write dependency ranges, but publication pins exact versions.

**Versions.** Artefacts and packages each carry their own semver. The diff check at validation enforces the bump rules: patch means no meaning change, minor means additive, major means breaking.

**Immutability.** A published `url|version` and a published `name@version` never change. Every node verifies the sha-256 and the signature before staging.

We chose this for three reasons:
- NPM packages are FHIR's native distribution format.
- CRMI manifests give the reproducible dependency set that historical interpretation needs.
- Signing plus content hashing lets packages travel over sync, USB or any untrusted medium.

## Considered Options

- **CRMI transaction Bundles as the package.** Rejected as the primary format, because they cannot carry non-FHIR configuration or fixtures. They remain an export option.
- **Our own package format.** Rejected: it would forfeit FHIR tooling and IG imports.
- **Floating dependency ranges at activation.** Rejected: a node could activate a set of packages that was never tested together.

## Consequences

- Package blobs travel through the content-addressed attachment store (ADR 0021).
- Nodes keep every package version ever activated in their scope for as long as any data references it.
- Packages never contain data migrations.
