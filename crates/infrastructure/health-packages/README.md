# health-packages

Layer: **infrastructure**. Published Definition packages on disk and in transit ([configuration.md](../../../docs/architecture/configuration.md) § Package format and § Signing; ADR 0024). It applies [`health-configuration`](../../domains/health-configuration)'s package rules, and verifies signatures through [`health-pki`](../health-pki).

- **`publish(files, signer, at)`** produces `Published`: the id `name@version`, the tarball, its `Digest` (sha-256) and the detached signature. In order, it:
  1. generates `.index.json`, the FHIR NPM index (version 2) of the top-level artefacts;
  2. refuses the package with `Error::NotPublishable` if any publication rule fails, an unpinned dependency among them;
  3. refuses it with `Error::Configuration` if any `configuration/` file fails its schema;
  4. writes a deterministic tarball;
  5. signs it.
- **Publication in steps,** for central's lifecycle (#88):
  - `pack(files)` packs without checking any rule, since a draft may break them until validation;
  - `check(packed, schemas)` applies the rules and schemas;
  - `sign(packed, schemas, signer, at)` checks again and signs;
  - `unpack(tgz, limits)` reads back a tarball central packed.

  `publish` is the three in one.
- **`verify(tgz, signature, expected, trust, limits, now)`** is what a node does before staging, for a package from sync or from USB. In order, it checks:
  1. the tarball's sha-256 against the one the activation names;
  2. the signature;
  3. that the archive unpacks safely;
  4. that `.index.json` lists exactly the package's artefacts;
  5. every publication rule, again;
  6. `configuration/` against this release's schemas.

  It returns `Verified`: the contents, the files, the digest and the signing time.

## The tarball

- **Writing is deterministic:**
  - entries go under `package/`, in path order;
  - every entry has mode 0644, owner 0 and mtime 0;
  - the gzip header has no name or time.

  So the same files always give the same sha-256.
- **Reading trusts nothing.** It refuses:
  - links, devices and FIFOs;
  - absolute paths, `..`, `.`, empty parts, backslashes and drive letters;
  - anything outside `package/`;
  - duplicate entries;
  - anything past the `Limits`: 50,000 entries, 512 MiB per file and 2 GiB in all by default.

## Terminology

`terminology::load(&mut terminology, files)` reads a package's CodeSystems, ValueSets and ConceptMaps into [`health-terminology`](../../domains/health-terminology)'s model (#90).

- **A ValueSet's `expansion`** is its pinned expansion. Nested `contains` are flattened, and abstract headings without a code are skipped.
- **A CodeSystem without `caseSensitive`** is taken as case-sensitive, the stricter reading.
- **ConceptMaps are R5:** a group's `source` and `target` lose any `|version`, and `noMap` is a target with no code, `not-related-to`.
- **Refused, never skipped:** a resource without its `url`, an unknown `content` or filter operator, a filter without its parts, a concept or listed code without its code, a group without its source and target, and an unknown relationship (`Error::Terminology`). Files that are not top-level JSON resources of those types are left alone.

## Configuration schemas

- **The schemas belong to the software,** not the package. A schema a package brought with it would let its author pass anything.
- **Each file names its schema:** every `configuration/` file is JSON, with a top-level `$schema` URI.
- **`Schemas`** is what the release reads, by URI. Each schema compiles once (`with`), and remote `$ref`s are not resolved. A file is refused (`Error::Configuration`, with each failing JSON pointer) when:
  - it is not JSON;
  - it names no `$schema`, or one this release does not know;
  - it does not validate.
- **Across releases:** a node verifies with its own `Schemas`, so a release that does not know a schema refuses a package that needs it, just as it refuses an unknown `formatVersion`.
- **`Schemas::platform()`** reads Form Layouts (#92). Override points and settings each bring their schema with the feature that reads them, and until then a package with such configuration is refused.

## The signature

A compact JWS with a detached payload (RFC 7515 Appendix F): `<header>..<signature>`, in `<name>-<version>.tgz.sig`.

- **Header:**
  - `alg`: `ES256`;
  - `typ`: `health-package+jws`;
  - `x5c`: the publication-signing certificate and any chain below the national root;
  - `iat`: the signing time.

  `crit` is refused.
- **Payload:** the tarball's 32-byte sha-256. It is not carried: the verifier hashes the tarball it has, so a changed byte anywhere fails the signature.
- **Trust:** `Trust::verify_publisher` checks the certificate at `iat`, not now. The certificate rotates yearly, and a node keeps every package it ever activated. A revoked certificate fails at any time, through the root's CRL, and an `iat` more than 5 minutes ahead is refused.
- **`PublicationSigner`** is the signing port, because the key is held in central's HSM or KMS. `LocalSigner` holds a PKCS#8 key in memory, for tests and tooling until the KMS adapter.

## Bump rules

`bump::changes(before, after)` finds every change between two versions of an artefact, with the bump each needs (metadata.md § Versioning; #88). The publication lifecycle's semver check (`health_configuration::lifecycle::semver::check`) then refuses an artefact whose own new version understates them.

- **Every resource:**
  - `version`, `date`, `meta` and `text` need nothing;
  - descriptive metadata (`name`, `title`, `status`, `publisher`, `description` and the like) needs a patch;
  - a changed `resourceType` or `url` is major;
  - extensions are compared by url: display-only ones (`DISPLAY_EXTENSIONS`: translations, rendering, item control, entry format) need a patch, and any other is major.
- **Questionnaire** items, matched by `linkId` at any depth:
  - removed is major; added is minor, or major if required;
  - `text` and `prefix` need a patch, and `initial` a minor;
  - `required`, `readOnly`, `repeats` and `maxLength` are major when they tighten and minor when they relax;
  - answer options: removed is major, added minor;
  - `type`, `answerValueSet`, `definition` (the extraction target), `code` and `enableWhen` are major.
- **CodeSystem** concepts, by `code` at any depth:
  - removed is major, added minor, and under another parent major;
  - `display`, `definition` and designations need a patch, and a concept's properties a minor;
  - property and filter definitions: removed is major, added minor.
- **ValueSet** `compose`:
  - `include` rules are matched by everything but their codes: removed or shrinking is major, added or growing minor;
  - `exclude` rules work the other way round;
  - `inactive` is minor from `false` to `true` and major otherwise, and `lockedDate` is major;
  - a stored `expansion` needs nothing.
- **StructureDefinition** elements, by `id`, in the differential (or the snapshot without one):
  - documentation needs a patch;
  - a higher `min`, lower `max`, stronger or different binding, added `fixed[x]`, `pattern[x]`, invariant, `mustSupport` or `maxLength` is major, and the reverse minor;
  - types and profiles: removed is major, added minor;
  - an element dropped from the differential relaxes, and one added constrains; one removed from a snapshot is major.
- **Anything else,** including every element without a rule and every other resource type, is major when it changes, so an unclassified change never passes as a patch.

## Not yet

- Staging and activation on a node are `health-edge-runtime`'s (#89): it fetches a tarball by the sha-256 its activation names (`Digest` reads back from hex), `verify`s it, and checks the package's `formatVersion` against the release's `FORMAT_VERSIONS`. Central stores packages as blobs through `health-publication`.

## Tests

- **Unit:** reading terminology: a package served through every operation, including a pinned ICD-10 subset and a ConceptMap with `noMap`; each malformed case refused; the default case sensitivity; and the core package's `template-modes` expanding from its CodeSystem. Also the bump rules, row by row, for each resource type; deterministic round trips, refused paths, hostile archives (traversal, absolute paths, a file outside `package/`, symlinks, hard links, FIFOs) and limits.
- **Integration (`tests/packages.rs`):** a real root and publication certificate.
  - A package publishes and verifies with its identity and index.
  - Dependency ranges and unpinned manifest canonicals are rejected at publication.
  - A tampered package is refused: any byte changed, another package under the signature, a sha-256 other than the activation's, or a forged signature.
  - A rogue root's signer, and another key under the real certificate, are refused.
  - A package signed while its certificate was valid still verifies years later, but one signed after it expired does not, and nor does one dated in the future.
  - A revoked certificate is refused.
  - A configuration file is refused when it has no `$schema`, names an unknown schema, fails validation (reported by JSON pointer), or is not JSON. A release without the schema refuses the published package, and an unusable schema is refused when built in.
- **Configuration schemas:** `Schemas::platform()` reads Form Layouts (`FORM_LAYOUT`, `schemas/form-layout-1.json`; #92). A layout names its Questionnaire `url|version` and arranges top-level items into titled sections, with widgets and hints. The schema admits nothing that could change or hide what is asked, and other configuration stays refused until its feature arrives.
