# health-packages

Layer: **infrastructure**. Published Definition packages on disk and in transit ([configuration.md](../../../docs/architecture/configuration.md) § Package format and § Signing; ADR 0024). It applies [`health-configuration`](../../domains/health-configuration)'s package rules, and verifies signatures through [`health-pki`](../health-pki).

- **`publish(files, signer, at)`** produces `Published`: the id `name@version`, the tarball, its `Digest` (sha-256) and the detached signature. In order, it:
  1. generates `.index.json`, the FHIR NPM index (version 2) of the top-level artefacts;
  2. refuses the package with `Error::NotPublishable` if any publication rule fails, an unpinned dependency among them;
  3. refuses it with `Error::Configuration` if any `configuration/` file fails its schema;
  4. writes a deterministic tarball;
  5. signs it.
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

## Configuration schemas

- **The schemas belong to the software,** not the package. A schema a package brought with it would let its author pass anything.
- **Each file names its schema:** every `configuration/` file is JSON, with a top-level `$schema` URI.
- **`Schemas`** is what the release reads, by URI. Each schema compiles once (`with`), and remote `$ref`s are not resolved. A file is refused (`Error::Configuration`, with each failing JSON pointer) when:
  - it is not JSON;
  - it names no `$schema`, or one this release does not know;
  - it does not validate.
- **Across releases:** a node verifies with its own `Schemas`, so a release that does not know a schema refuses a package that needs it, just as it refuses an unknown `formatVersion`.
- **`Schemas::platform()`** is empty for now. Form Layouts, override points and settings each bring their schema with the feature that reads them, and until then a package with configuration is refused.

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

## Not yet

- Storing packages as blobs, activation and staging come with the publication lifecycle (#88) and scoped activation (#89).

## Tests

- **Unit:** deterministic round trips, refused paths, hostile archives (traversal, absolute paths, a file outside `package/`, symlinks, hard links, FIFOs) and limits.
- **Integration (`tests/packages.rs`):** a real root and publication certificate.
  - A package publishes and verifies with its identity and index.
  - Dependency ranges and unpinned manifest canonicals are rejected at publication.
  - A tampered package is refused: any byte changed, another package under the signature, a sha-256 other than the activation's, or a forged signature.
  - A rogue root's signer, and another key under the real certificate, are refused.
  - A package signed while its certificate was valid still verifies years later, but one signed after it expired does not, and nor does one dated in the future.
  - A revoked certificate is refused.
  - A configuration file is refused when it has no `$schema`, names an unknown schema, fails validation (reported by JSON pointer), or is not JSON. A release without the schema refuses the published package, and an unusable schema is refused when built in.
