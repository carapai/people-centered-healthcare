# CI runs pinned, least-privilege workflows behind a merge queue, and a release is either fully signed with provenance or it fails

**Workflows.** The nine workflows of §111 gate `main` through a merge queue, with squash merges and Conventional Commit PR titles.
- **Approvals:** one approval, plus CODEOWNERS for ADRs, architecture docs, architecture checks, security, sync and `.github/`. ADR changes need two approvals.
- **Actions** are pinned to full commit SHAs by tooling (Renovate or `pinact`), never by hand.
- **Permissions** default to `contents: read` per job.
- **`pull_request_target`** never checks out PR code.
- **`zizmor`** lints the workflows.
- **Cloud access** uses GitHub OIDC federation, never long-lived secrets.

**Releases.** There is one platform SemVer, with release-please changelogs.
- **Trigger:** a tag runs `release.yml` in a protected `release` environment, which needs two release-manager approvals.
- **Signing:**
  - Authenticode via Azure Artifact Signing for Windows;
  - the Tauri updater key from KMS;
  - GPG for AppImage and the apt repository;
  - a release-signing key for edge update packages, separate from metadata publication signing;
  - `cosign` keyless signing for the central OCI image.
- **Provenance:** SLSA provenance attestations and CycloneDX SBOMs accompany every release.
- **Signed or failed.** If any signing step lacks its configuration, the release job fails. PR builds are labelled `unsigned-dev`.

**Build matrix:** Windows 10/11 x64 and Ubuntu 22.04/24.04 x64 for the edge and desktop, Linux x64 OCI for central, and the static web bundle. There are no macOS builds, because macOS is not a supported target.

**Licences.** Allowed: MIT, Apache-2.0, BSD, ISC, Unicode, Zlib and MPL-2.0, plus SurrealDB's BSL 1.1 as a documented exception. Copyleft needs an ADR.

We chose this because §115 requires reviewed, pinned actions and forbids pretending that releases are signed. Edge and desktop binaries run unattended in facilities, so a compromised build pipeline would reach every clinic.

## Considered Options

- **Version tags for actions (`@v4`).** Rejected: they are mutable, and a supply-chain risk.
- **Publishing unsigned releases while signing is being set up.** Rejected by §115.
- **Separate version numbers per artifact.** Rejected: the sync handshake and support policy reason about one platform version (N-1).

## Consequences

- Signing accounts (Azure Artifact Signing, the KMS updater key, GPG) must exist before the first release.
- Renovate maintains SHA pins and groups updates weekly.
