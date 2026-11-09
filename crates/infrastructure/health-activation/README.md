# health-activation

Package activation on a node (#89; [configuration.md](../../../docs/architecture/configuration.md) § Distribution and activation on nodes; ADR 0025). It is shared by the edge (`health-edge-runtime`, after each sync round) and central (`central-server`, every minute).

## `activate`

- **The wanted set:** what the node's activation records give it now along its scope chain (`health_configuration::activation`). Central's chain is national only.
- **Staging:** each wanted package not yet staged is fetched by its sha-256 (an edge fetches from central; central holds what it published) and verified with `health_packages::verify`:
  - the sha-256 the activation names;
  - the publication signature against the national root;
  - a safe archive;
  - the publication rules and the release's schemas.

  The tarball is kept in the blob store, which retains every version ever activated, and the package is recorded as staged with its `package.json`. A package that fails is reported, never staged.
- **Switch:** the set is checked (staged, exact dependencies, `minSoftware`, `formatVersion`). The node switches to it in one transaction, or keeps the previous set. Either way a `Report` is kept, and a failure raises `package-activation-failed`.

## `definitions`

- **`write_definitions(store, blobs, active)`:** central writes each active package's definitional resources into its store. These are ActivityDefinition, CodeSystem, ConceptMap, Library, PlanDefinition, Questionnaire, SearchParameter, StructureDefinition and ValueSet.
  - Clients read them there, and they sync down to every node in the packages lane.
  - Each is stored under a UUIDv5 of its `url|version` (`definition_id`). Every version is its own resource: a rollback finds the earlier one still there, and records keep the version they were written against.
  - A resource already written is left as it is.
- **Only central writes them:** nodes below receive them by sync.

## Tests

- **Edge** (`health-edge-runtime/tests/sync.rs`, over HTTPS in process):
  - an activated package is fetched, verified, staged and switched to;
  - a failed activation keeps the previous set and reports;
  - a forged signature never stages.
- **Here:** each active version's definitions are written once under their own ids, and a set naming an unstaged package is refused.
