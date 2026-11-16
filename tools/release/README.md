# tools/release

The release pipeline's scripts ([deployment.md](../../docs/architecture/deployment.md) § Releases; ADR 0045). `.github/workflows/release.yml` runs them on a `v<version>` tag, and `.github/workflows/release-check.yml` checks them on pull requests and weekly, with throwaway keys.

| File | |
|---|---|
| `check-signing.sh` | **Signed or failed.** Every release job runs it first. It fails, naming them, when any signing setting below is missing, so nothing is built or published |
| `check-version.sh` | The tag must name the version central, the edge and the desktop carry, and the tagged commit must be on `main` |
| `sign-edge-package.sh` | Signs an edge update package with the release-signing key in Key Vault, then verifies it with the edge's own `facility-edge update verify` against the pinned public key |
| `sbom.sh` | CycloneDX SBOMs: `central-server` and `facility-edge` from `cargo-cyclonedx`, the Bun workspace from cdxgen. Each must list what it obviously contains |
| `sign-updater.sh` | Signs the desktop's updater artifacts with the Tauri updater key, read from Key Vault into that process alone, after Authenticode and GPG, then checks each against the pinned public key |
| `verify-updater-signature.sh` | That check: a Tauri (minisign) signature against a public key, with `openssl` |
| `import-gpg-key.sh` | Imports the release GPG key from Key Vault into the job's keyring, and checks its fingerprint |
| `desktop-linux.sh`, `desktop-linux-deps.sh` | The AppImage (GPG-signed, the signature checked to be the release key's) and the deb, and the apt repository |
| `apt-repo.sh` | A flat apt repository for the deb, its `Release` clear-signed and detach-signed, checked with `gpgv` |
| `tauri.offline.json` | The configuration overlay for the offline Windows installer, which carries WebView2, for USB installs |
| `check-workflow.ts` | Holds `release.yml` to its rules: every job in the `release` environment and gated, every setting it reads checked by the gate, only `publish`, which needs every other job, publishing, and the updater key never in a step's environment or Vite's |
| `test/az`, `test/throwaway-keys.sh` | A stand-in for the two Key Vault calls, and throwaway keys in it, for the dry run |

## What a release publishes

| Artifact | Signing |
|---|---|
| `facility-edge-<v>-linux-x64.tar.gz` and `-windows-x64.tar.gz`, each with `.sig` | The release-signing key (ECDSA P-256 in Key Vault); the Windows binary inside is Authenticode-signed first |
| The central image, `ghcr.io/<repository>/central-server:<v>` | `cosign` keyless, plus a provenance attestation pushed to the registry |
| `web-<v>.tar.gz` | Provenance |
| `central-server.cdx.json`, `facility-edge.cdx.json`, `health-desktop.cdx.json`, `web.cdx.json` | Provenance |
| `health-platform-<v>-x64-setup.exe` | Authenticode (the installer and the app inside it), and the updater key (`.sig`): the Tauri updater installs it |
| `health-platform-<v>-x64-offline-setup.exe` | Authenticode; it carries WebView2, for USB installs |
| `health-platform-<v>-x86_64.AppImage` | GPG, embedded, and the updater key (`.sig`) |
| `health-platform-<v>-amd64.deb`, and `apt-repo-<v>.tar.gz` holding it | The apt repository's `InRelease` and `Release.gpg`, by the GPG key, whose public half it carries |
| `SHA256SUMS` | Provenance |

Every file has a SLSA provenance attestation (`gh attestation verify <file> --repo <repository>`).

### The desktop's order of signing

Authenticode and GPG change a file's bytes, and the updater checks the bytes it downloads, so the updater signs last:

1. `tauri build --no-bundle` builds the app, from a webview bundle built in an earlier step: no Vite runs where a key is present.
2. Authenticode signs the app, then both NSIS installers are bundled around it and signed in turn. The job checks every signature, which also catches a bundling step that rebuilt the app.
3. The AppImage is signed with GPG as it is built.
4. `sign-updater.sh` signs the standard installer and the AppImage for the updater. `tauri.conf.json` keeps `createUpdaterArtifacts` off, so the bundler never signs too early.

## Setting up the `release` environment

Done once, by people with the accounts. Until every setting exists, a release fails at its first step and publishes nothing.

1. **The environment:** create `release` in the repository settings. Require two release managers as reviewers, and allow only `v*` tags to deploy to it.
2. **Azure OIDC:** an app registration (or user-assigned managed identity) with a federated credential for `repo:<owner>/<repository>:environment:release`. No client secret.
3. **Azure Artifact Signing:** an account and a certificate profile, with the *Artifact Signing Certificate Profile Signer* role for the identity above.
4. **Key Vault**, with the identity allowed to sign with keys and read secrets:
   - an **EC P-256 key** for edge packages, non-exportable;
   - the **Tauri updater key** and its password, as two secrets (ideally generated at the key ceremony, with an offline backup: losing it strands every installed desktop);
   - the **GPG private key** (armoured) for the AppImage and the apt repository, as a secret.
5. **Environment variables** (variables, not secrets: none of them is a key):

| Variable | |
|---|---|
| `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_SUBSCRIPTION_ID` | The OIDC identity |
| `ARTIFACT_SIGNING_ENDPOINT`, `ARTIFACT_SIGNING_ACCOUNT`, `ARTIFACT_SIGNING_PROFILE` | e.g. `https://weu.codesigning.azure.net/` |
| `KEY_VAULT_NAME` | |
| `EDGE_RELEASE_KEY` | The EC key's name |
| `EDGE_RELEASE_PUBLIC_KEY` | Its public half as edges pin it: hex SEC1, e.g. `az keyvault key download … --encoding DER` then the last 65 bytes as hex |
| `UPDATER_KEY_SECRET`, `UPDATER_KEY_PASSWORD_SECRET` | The secrets' names |
| `UPDATER_PUBLIC_KEY` | The updater's public key, as `tauri signer generate` prints it |
| `GPG_KEY_SECRET` | The secret's name |
| `GPG_KEY_FINGERPRINT` | The key's fingerprint |

## Not yet

- **release-please:** tags are pushed by hand. Its pull requests, opened with the workflow token, would not run CI, so it needs a GitHub App token first.
- **Publishing beyond GitHub:** the apt repository is built and signed but not hosted, and the updater endpoint and the edge package channel at central serve what the GitHub release holds once they exist (#136).
