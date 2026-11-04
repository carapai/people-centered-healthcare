# health-terminology

Layer: **domain** (Terminology & Metadata). The terminology operations every node serves over its own data, with no external terminology server (#90; [terminology.md](../../../docs/architecture/terminology.md); ADRs 0003, 0024). The crate is pure: adapters read CodeSystems, ValueSets and ConceptMaps from packages into its model.

- **`Terminology`** holds the node's CodeSystems, ValueSets and ConceptMaps, by canonical URL and version.
  - A reference without a version resolves to the latest version held, compared as semver when both versions are semver (`1.10.0` after `1.9.0`).
- **`lookup(system, version, code)`** (`$lookup`) returns the concept's display, definition, designations and properties, and the codes directly above and below it.
  - The hierarchy comes from nested concepts and from `parent` properties.
  - A code system that is not case-sensitive compares codes without case.
- **`subsumes(system, version, a, b)`** (`$subsumes`) returns `Equivalent`, `Subsumes`, `SubsumedBy` or `NotSubsumed`.
- **`expand(value_set, request)`** (`$expand`):
  - **The pinned expansion** a package ships with the ValueSet is used as it is, with no code system needed.
  - **Otherwise the compose is computed:**
    - an `include` selects a whole code system, listed codes, or codes passing every filter;
    - imported value sets intersect, and an `exclude` removes codes;
    - `compose.inactive = false` drops inactive codes (an `inactive` property that is true, or a `retired` status).
  - **The request** can drop inactive codes (`active_only`), filter by text in a code or display ignoring case, and page with `offset` and `count`. `total` counts the codes before paging.
  - **Filters:**
    - on `concept`: `is-a`, `descendent-of`, `is-not-a`, `generalizes`, `child-of`, `in` and `not-in`;
    - on a property: `=`, `in`, `not-in` and `exists`.
- **`validate_code(value_set, system, code, display)`** and **`validate_in_code_system`** (`$validate-code`) return:
  - `Valid` with the code's display;
  - `WrongDisplay`, when the display given is neither the display nor a designation;
  - `Invalid`.
- **`translate(concept_map, system, code, target_system)`** (`$translate`) returns each target with its relationship, from one ConceptMap or from every one held.

## Offline, so fail closed

What would need another terminology server is an error, never an empty answer:
- a code system that is not held, when a rule selects all of it or filters it;
- a filter on a code system held only in part (not `complete`);
- a value set or concept map that is not held;
- a listed code the held code system does not have;
- a `regex` filter;
- a value set that includes itself.

Codes listed for a code system that is not held are taken as listed. That is how a pinned subset of a large external code system, such as ICD-10, arrives.

## Not yet

- **The FHIR operation endpoints** (#90 part 2b). `health-packages::terminology` reads packages into this model.
- **Binding validation** with these operations is tier 4 (#91).

## Tests

The unit tests run over an ANC danger-signs code system with a nested hierarchy, an inactive code, a designation and a property. They cover:
- **Lookup:** display, designations, properties, parents and children, and unknown codes, code systems and versions.
- **Subsumption:** all four answers, including a flat code system with `parent` properties and case-insensitive codes.
- **Expansion:**
  - inactive codes, text filter and paging;
  - each filter operator;
  - listed codes with the value set's display;
  - imports intersecting and exclusions removing;
  - a listed code that does not exist;
  - everything that fails closed, and the pinned expansion used without the code system.
- **Validation:** membership, a designation as a valid display, a wrong display, and validation against a code system.
- **Versions:** the latest by semver, or a pinned one.
- **Cycles** are refused.
- **Translation** through one map or all of them, by target system, including an unmapped code.
