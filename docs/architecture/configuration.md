# Configuration, packages and publication

Status: **accepted** (ADRs 0024, 0025, 0026). Produced while resolving the Wayfinder ticket [How are terminology, metadata and configuration packaged and published?](https://github.com/carapai/people-centered-healthcare/issues/23). Related layers: [terminology.md](terminology.md) and [metadata.md](metadata.md). Background: [docs/research/metadata-terminology.md](../research/metadata-terminology.md).

## Three layers

| Layer | Module | Defines | Example |
|---|---|---|---|
| Terminology | terminology | Meaning | ValueSet of malaria test results |
| Metadata | metadata | Clinical structure | ANC visit Questionnaire, its profile and Extraction Mapping |
| **Configuration** | configuration | How the platform behaves in a place | Form Layout for that Questionnaire, which services a facility offers, role → permission mappings, snapshot lifetime |

- **Configuration can't change clinical meaning.** It references metadata and terminology, and never changes their meaning.
- **Presentation is kept apart (§42).** A **Form Layout** is a configuration artefact keyed to a Questionnaire `url|version`. The Questionnaire itself keeps only clinical structure.
  - **The format** (`$schema` `https://carapai.github.io/people-centered-healthcare/fhir/schema/form-layout/1`; #92): `url`, `version`, `questionnaire`, titled `sections` of top-level linkIds, and per-item `widget` (`input`, `textarea`, `radio`, `select`, `checkbox`) and `hint`.
  - **No hiding:** a layout cannot hide a question. Items it does not place are shown after its sections.

Configuration artefacts are JSON documents in a package's `configuration/` folder. Each is validated against a JSON Schema that the platform release ships. Each carries its own `url` and `version`, and the manifest Library pins it as a `composed-of` `relatedArtifact` (ADR 0049).

## Package format (ADR 0024)

```text
ug.moh.anc-1.4.0.tgz
└── package/
    ├── package.json            name, version, fhirVersions ["5.0.0"], dependencies (exact after publication),
    │                           platform: { minSoftware, formatVersion }, kind
    ├── Library-manifest.json   CRMI-style manifest: every dependency url|version, pinned ValueSet expansions
    ├── *.json                  FHIR artefacts (Questionnaire, Extraction Mapping, PlanDefinition, …)
    ├── configuration/          Form Layouts, override points, settings (JSON Schema validated)
    ├── tests/                  extraction fixtures, simulation cases
    └── .index.json
ug.moh.anc-1.4.0.tgz.sig        detached signature (publication-signing certificate)
```

| Kind | Contents | Typical approver |
|---|---|---|
| National core | Profiles, base terminology, SearchParameters | MoH standards team (two approvers) |
| Terminology | Large code systems and expansions (ICD, LOINC, …) | Terminology lead (two approvers) |
| Program | Forms, mappings, workflow and program definitions for ART, TB, ANC, … | Program clinical leads (two approvers) |
| Country configuration | Roles and permissions, deployment settings | Platform admin (one approver) |
| Override | District or facility configuration at declared override points | See [Overrides](#overrides-adr-0026) |

- **Identity.** A package is identified by `name@version` and its sha-256.
- **Transport.** Packages travel as blobs through the content-addressed store (ADR 0021).
- **Pinned dependencies.** Authors write semver ranges. Publication resolves them and pins exact versions in `package.json` and the manifest.
- **Configuration schemas** (decided in #86):
  - Every `configuration/` file is JSON, and names its schema with a top-level `$schema` URI.
  - The schemas belong to the software release, as format versions do, never to the package.
  - Publication and every node refuse a file whose schema they do not know or that does not validate.

## Lifecycle (§45; ADR 0025)

```mermaid
stateDiagram-v2
  [*] --> Draft
  Draft --> Validation: submit
  Validation --> Draft: fails
  Validation --> Simulation: passes
  Simulation --> Draft: fails
  Simulation --> Review
  Review --> Draft: changes requested
  Review --> Approval
  Approval --> Published: approved and signed
  Published --> Active: activation record (scope, effective date)
  Active --> Superseded: newer activation in same scope
  Superseded --> Active: rollback = re-activation
  Published --> Retired: no scope activates it
  Superseded --> Retired: no scope activates it
```

| Step | What happens |
|---|---|
| Draft | Authored in the Clinical Metadata Studio at **central only**. Drafts never sync |
| Validation | Automated: FHIR validation of every artefact; every profile carries a snapshot, compiles, and uses only constructs the tier 3 validator supports (ADR 0053); dependency resolution; FHIRPath type-checks; JSON Schema for configuration; the semver diff check; extraction fixtures; Owner opt-in check for mapping targets; override-point check; the R4 → R5 conversion report for imports |
| Simulation | `tests/` cases run in a sandbox: form fill and extraction, workflow paths, indicator outputs. The tooling UX is in the map's Studio fog |
| Review | A reviewer who is not the author |
| Approval | Two distinct approvers (neither the author) for clinical content (national core, terminology, program); one for configuration |
| Publication | Signed with the publication-signing certificate; immutable; CRMI `active` |
| Activation | A global record with package `name@version`, scope (national, district, facility or program) and effective date. Publication never activates anything by itself |
| Supersession | A newer activation in the same scope. The old version stays retrievable, and becomes `retired` once nothing activates it |

Every transition is a lifecycle record owned by configuration, and every transition is audited.

## Distribution and activation on nodes

```mermaid
sequenceDiagram
  participant C as Central
  participant E as Facility Edge Node
  participant D as Desktop
  C->>E: activation record (metadata lane)
  E->>C: fetch package blob(s) by hash
  E->>E: verify signature + sha-256, stage
  E->>E: check dependencies, minSoftware, formatVersion
  alt incompatible or missing dependency
    E-->>C: activation failed (reason), keep previous set
  else ok
    E->>E: at effective time: switch active set atomically
    E->>E: reindex if SearchParameters changed
    E->>D: activation record + package (desktop lane)
  end
```

- **Active set.** A node's active set is the merge of activations along its scope chain: national → district → facility, plus program activations for the facility.
- **Offline.** Packages can be applied from USB on an offline edge. The signature is verified the same way.
- **Software compatibility.** Each package declares `minSoftware` and `formatVersion`, and each software release declares the format versions it supports. Activation is refused on a mismatch, which is the same check as the sync handshake (synchronization.md).
- **Retention.** Nodes retain every package version ever activated in their scope for as long as referencing data exists (metadata.md).
- **In code (#89):**
  - **Domain** (`health_configuration::activation`): the active set merged along a node's scope chain, the checks before a switch, and a switch that is whole or keeps the previous set and reports.
    - For one package, the most specific scope wins, and within a scope the latest issued activation in effect.
    - Dependencies must be active at exactly their pinned versions.
  - **At central:** `central-server package activate <name@version> --scope … --by …` issues the record (`Publications::activate`). `package activations` lists them.
  - **To nodes:** each record is kept in `package_activation` (migration `0049_package_activation`) and goes down as an operation in the packages lane, to every node, in pulls and re-seeds alike. A node never sends one up.
  - **On a node** (`health-activation`, shared by the edge after each sync round and central every minute).
  - **On an edge**, after each sync round, the node works out the set its records want along its chain (`HEALTH_DISTRICT`, `HEALTH_FACILITY`, `HEALTH_PROGRAMS`).
    - It fetches each missing tarball from central's blob store by sha-256 and verifies it against the national root. It keeps the tarball (retention) and records the package as staged (migration `0050_package_staging`).
    - It then switches in one transaction or keeps the previous set. Each attempt is reported, and a failure raises `package-activation-failed`.
    - Central keeps tarballs in `HEALTH_FILES_DIR`, the store sync serves.
    - Its reports go up to central (`POST /sync/v1/activation-reports`; migration `0051_activation_reports`). Central keeps them per node, raises `package-activation-failed` for a failure, and lists them with `central-server package reports`.
  - **On central** (`HEALTH_NATIONAL_ROOT_FILE` with `HEALTH_FILES_DIR`), the national set is activated from central's own blob store.
    - When it changes, and at startup, the active packages' definitional resources are written into central's store (`health_activation::definitions`), each under a UUIDv5 of its `url|version`.
    - Clients read them there, and they sync down to every node in the packages lane. A rollback finds the earlier version still there.
    - The same set gives central's FHIR server its profiles and terminology. Its StructureDefinitions, compiled, validate writes (tiers 3–4). Its CodeSystems, ValueSets and ConceptMaps answer the terminology operations and bindings.
    - A set whose profiles do not compile keeps the previous ones and raises `package-activation-failed`. Without `HEALTH_NATIONAL_ROOT_FILE`, central validates no profiles, as before.
  - **Not yet:** the active set's SearchParameters (with their reindex) and workflows, and desktops. `HEALTH_DEV_FHIR_SEED` and `HEALTH_DEV_WORKFLOW_DEFINITIONS` stay until then.

## Rollback

- **Rollback is re-activation.** It is a new activation record pointing at an earlier version, so nothing is mutated or deleted.
- **Existing data is unaffected.** Records written under the rolled-back version keep their pinned versions and are still interpreted with them.
- **Always possible,** because packages never contain data migrations.

## Overrides (ADR 0026)

- **Override points.** Only configuration keys declared as **override points** by the base package can be overridden. Examples are layout, section visibility, which Healthcare Services and programs are active, worklist and queue settings, and local settings.
- **Never overridable:** profiles, terminology, Extraction Mappings, workflow definitions and clinical rules.
- **Layering.** Overrides are layered national → district → facility and merged key by key; the most specific value wins.
- **Approval.** Presentation-only overrides need the facility admin alone. Clinically visible overrides, such as hiding a form section or disabling a program form, also need a district or central reviewer.
- **Lifecycle.** Overrides follow the same lifecycle as any package.
- **Base upgrades.** A base package's major version may remove override points. Validation flags any override that becomes orphaned before the new base can be activated in that scope.

## Signing

- **Certificate.** Publication uses a dedicated **publication-signing certificate** issued by the national root at a ceremony (ADRs 0008, 0058). It is not a CA, its only EKU is code signing, and it is separate from every TLS certificate.
- **Rotation.** It is rotated yearly, and its key is held in central's HSM or KMS.
- **Format** (decided in #86): the `.sig` file is a compact JWS with a detached payload (RFC 7515 Appendix F).
  - Its header has `alg` `ES256`, `typ` `health-package+jws`, `x5c` (the certificate and any chain below the root) and `iat`, the signing time.
  - The payload it signs is the tarball's sha-256.
  - Nodes check the certificate at `iat`, so a package outlives the certificate's yearly rotation. A revoked certificate fails at any time.
  - [health-packages](../../crates/infrastructure/health-packages/README.md) writes and verifies it.
- **Trust.** Nodes trust it through the root they already hold. Revocations arrive in the revocation lane.
- **Custody.** Who holds the national root is in the map's PKI governance fog.
