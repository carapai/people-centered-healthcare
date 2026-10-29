# Deployment: CI/CD, releases, environments, observability and recovery

Status: **accepted** (ADRs 0045, 0046, 0047). Produced while resolving the Wayfinder ticket [What are the CI/CD, release and production-operations rules?](https://github.com/carapai/people-centered-healthcare/issues/31). Spec: MASTER_PROMPT §92–94, §109–115. Related docs:
- the test strategy: [testing.md](testing.md);
- architecture checks: [dependency-rules.md](dependency-rules.md);
- edge backups: [backup-and-recovery.md](backup-and-recovery.md);
- desktop packaging: [desktop-client.md](desktop-client.md).

## Git and merge rules (§110)

- **`main`:** protected, behind a **merge queue**, squash merges, linear history.
- **Commit format:** Conventional Commits, enforced on PR titles. Branches are short-lived.
- **Reviews:**
  - one approval;
  - **CODEOWNERS** review for `docs/adr/`, `docs/architecture/`, `tools/architecture-checks/`, security, sync crates and `.github/`;
  - ADR changes need **two** approvals.
- **Dependencies:**
  - lockfiles are committed (`Cargo.lock`, `bun.lock`);
  - Renovate groups updates weekly, auto-merges dev-dependency patches after checks, and raises security updates immediately.

### Git and merge rules in code

- **`tools/github/configure-repository.sh`** applies the settings by API, idempotently (#48):
  - squash-only merging, with the PR title as the commit subject, and head branches deleted on merge;
  - secret scanning with push protection;
  - a `main` ruleset with no deletion or force push, linear history, a PR with one approval, code-owner review and resolved conversations, and every required check passing on an up-to-date branch.
- **`.github/CODEOWNERS`** covers `docs/adr/`, `docs/architecture/`, `MASTER_PROMPT.md`, `tools/architecture-checks/`, `.github/`, `deny.toml`, `renovate.json`, and the security and sync crates by name, before they exist.
- **`pr-title.yml`** checks that the PR title is a Conventional Commit, since squash merges use it as the commit subject.
- **`adr-approvals.yml`** requires a second approval when `docs/adr/` changes; GitHub rulesets cannot set approval counts per path.
- **Bypass.** Repository admins may bypass the ruleset through a PR only. While the maintainer is the only reviewer, GitHub does not let them approve their own PR, so merges use this bypass, which GitHub records. It is removed once a second reviewer exists.
- **Merge queue.** It needs an organization-owned repository. The workflows already listen for `merge_group`, so the queue can be turned on after a transfer. Until then, the strict status-check policy (an up-to-date branch) stands in for it.
- **Public repository.** Rulesets, code scanning, dependency review and push protection on this account need a public repository; so the repository is public (#48).

## Workflows (§111–113)

| Workflow | Trigger | Jobs |
|---|---|---|
| `rust-ci.yml` | PR, merge queue, main | `cargo fmt --check`; `cargo clippy -D warnings`; build; `cargo nextest` (unit, domain); doc tests; `cargo doc -D warnings`; `tools/architecture-checks`; `cargo-deny`; `cargo-llvm-cov` ≥ 80% lines on domain and application crates |
| `frontend-ci.yml` | PR, merge queue, main | `bun install --frozen-lockfile`; `tsc`; lint (including the `src/platform` import rule); `panda codegen`; Vitest; component tests; production build; bundle scan (no `@tauri-apps`, no devtools); regenerated OpenAPI/TS types must match the committed ones |
| `integration-tests.yml` | PR (fast tier), nightly (full) | Repository tests against the pinned SurrealDB; `EXPLAIN` plan tests; migrations, then `INFO FOR DB` compared with the schema snapshot; export → import round trip; API contract tests; workflow scenarios; extraction and pricing fixtures; multi-node sync simulation |
| `security.yml` | PR, weekly | `cargo-deny` advisories; `osv-scanner` on `bun.lock`; `gitleaks`, plus GitHub secret scanning with push protection; `zizmor`; CodeQL for JS/TS (for Rust only once GA; unverified today) |
| `dependency-review.yml` | PR | `actions/dependency-review-action`, with the licence allowlist |
| `desktop-build.yml` | PR (unsigned), release (signed) | Tauri on Windows x64 and Ubuntu 22.04 x64 |
| `edge-build.yml` | PR (unsigned), release (signed) | `facility-edge` on Windows x64 and Ubuntu x64, built separately from Tauri |
| `release.yml` | Release tag | Build matrix, signing, SBOMs, provenance, publishing (below) |
| `documentation.yml` | PR touching `docs/` or `*.surql` | markdownlint; `lychee` link check; Mermaid render; `surreal validate` plus `surql-fmt --check`; ADR numbering and index check |

### Workflows in code

Five workflows are in `.github/workflows/` (#47); every action is pinned to a full commit SHA by `pinact`, and Renovate (`renovate.json`) keeps the pins and groups updates weekly, with security fixes at any time.

- **`rust-ci.yml`** runs the table's jobs, plus a `java-differential` job (the HL7 validator as a CI oracle, [fhir-r5-model.md](fhir-r5-model.md) § The HL7 Java validator differential) and a `fhir-corpus` job. That job fetches the R5 core package and examples with pinned SHA-256s (`tools/fhir/fetch-r5-core.sh`, `fetch-r5-examples.sh`), checks the element table, and runs the corpus and validation-budget tests in release. Coverage measures the crates whose `layer` is `domain` or `application`.
- **`frontend-ci.yml`** runs install, `panda codegen`, `tsc`, lint with the boundary rule, the production build and the bundle scan. Vitest and component tests join it when the client has tests ([#173](https://github.com/carapai/people-centered-healthcare/issues/173)); the OpenAPI/TS type diff comes with [#50](https://github.com/carapai/people-centered-healthcare/issues/50).
- **`security.yml`** and **`dependency-review.yml`** run as specified. CodeQL, dependency review and push protection need the repository to be public on this account (or GitHub Advanced Security). zizmor reports in the log instead of uploading SARIF.
- **`documentation.yml`** runs on every PR, since its jobs are required checks: markdownlint (`.markdownlint-cli2.jsonc`), lychee (`lychee.toml`; this private repository's own URLs are excluded, and relative links are checked), Mermaid rendering (`tools/docs/render-mermaid.sh`), `surreal validate` with the pinned 3.3.5 binary, `surql-fmt --check`, and `tools/docs/check-adr-index.sh`. An applied migration is immutable (the runner records its SHA-256 and refuses a changed file), so `surql-fmt` checks a migration only in the PR that adds it, and never asks for an applied one to be reformatted.
- **Not yet:** `integration-tests.yml`, `desktop-build.yml`, `edge-build.yml` and `release.yml` come with their tasks; branch protection and CODEOWNERS are below (§ Git and merge rules in code).

### Architecture enforcement (§109)

| Detects | Check |
|---|---|
| Circular dependencies, forbidden imports | `tools/architecture-checks`; oxlint `no-restricted-imports` and `apps/client/scripts/check-boundaries.ts` |
| Cross-domain database access | Repositories declare the tables they own. A test asserts that every table has exactly one owner, and that each repository's queries touch only its own tables or the shared infrastructure tables |
| Duplicate resource ownership | An ownership registry in code, with a doc-sync test against `resource-ownership.md` |
| Desktop imports in web builds | Lint rule, plus a scan of the production bundle |
| Contract inconsistencies | Regenerated OpenAPI and TS types diffed against the committed ones; fixtures validated against FHIR profiles |
| Missing migrations | Fresh-database migration plus a schema snapshot comparison |
| Required tests | Coverage floor on domain and application crates; fixture requirements as publication gates for definitions |

### Workflow security (§115; ADR 0045)

- **Pinning.** Third-party actions are pinned to **full commit SHAs** with version comments, set by Renovate or `pinact`, never hand-typed.
- **Permissions.** Each job declares `permissions:`, defaulting to `contents: read`.
- **No `pull_request_target`** with a PR checkout.
- **Cloud access** uses GitHub OIDC federation (Azure Artifact Signing, KMS). There are no long-lived cloud secrets.

### Licences

- **Allowed:** MIT, Apache-2.0, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0.
- **SurrealDB's BSL 1.1:** an explicit, documented exception.
- **GPL and AGPL** need an ADR.

## Build matrix (§114)

| Artifact | Targets | Package |
|---|---|---|
| Facility Edge Node | Windows 10/11 x64 | MSI that registers the Windows service |
| | Ubuntu 22.04 / 24.04 x64 | deb with a systemd unit |
| Desktop | Windows 10/11 x64 | NSIS (bootstrapper and offline-WebView2 variants) |
| | Ubuntu 22.04 / 24.04 x64 | AppImage, deb |
| Central | Linux x64 | OCI image |
| Web | — | Static bundle (served by central, embedded in the edge) |

macOS is not a supported target for any artifact in v1, so no macOS build is produced or claimed.

**Build runners.** Release builds run on runners with at least **8 GiB of RAM** (ADR 0050). The serde code for the R5 `Resource` enum peaks at about 6.2 GiB of rustc memory, and it is built in `health-fhir-models`. The facade's `parse_resource` and `to_json` keep it from being rebuilt in every crate that parses resources. The model crate is cached between CI runs.

## Releases (ADR 0045)

```mermaid
flowchart LR
  T[Release tag<br/>release-please] --> A{release environment<br/>2 release-manager approvals}
  A --> B[Build matrix]
  B --> S[Sign]
  S --> P[SBOM + SLSA provenance]
  P --> U[Publish: updater endpoint, apt repo, OCI registry, edge package channel]
  S -. any signing config missing .-> F[Fail: nothing published]
```

- **One platform SemVer** across central, edge, desktop and web, with a changelog from Conventional Commits.
- **Signing:**

  | Artifact | Signing |
  |---|---|
  | Windows edge and desktop installers | Authenticode via Azure Artifact Signing |
  | Tauri updater artifacts | Updater key in KMS |
  | AppImage, apt repository | GPG |
  | Edge update packages | Release-signing key, separate from the metadata publication key (ADR 0024) |
  | Central OCI image | `cosign` keyless (Sigstore) |

- **Attestations:** SLSA provenance (`actions/attest-build-provenance`), plus CycloneDX SBOMs for Rust (`cargo-cyclonedx`) and Bun, attached to every release.
- **Signed or failed.** The release job **fails** if any signing configuration is missing. PR builds are labelled `unsigned-dev`.

## Environments and rollout

| Environment | Contents |
|---|---|
| CI | Ephemeral |
| Staging | Central, plus simulated edges and desktops on VMs, with synthetic data |
| Pilot | A small set of real facilities |
| Production | All facilities, in district waves |

- **Staged rollout.** Each node's allowed version is a per-facility record at central, used by both the desktop updater endpoint and the edge package channel.
- **Promotion between waves** requires the previous wave's telemetry to be within thresholds: error rate, sync lag, failed migrations.
- **Release block.** A release is blocked if any migration lacks its export → import round-trip test.

## Observability (ADR 0047)

- **Collection:** OpenTelemetry from central, edges, desktops and frontend error reporting. Edges buffer locally and ship when online (facility-edge.md).
- **Stack:** self-hosted **in-country**: an OTel collector, Prometheus-compatible metrics, Loki, Tempo and Grafana.
- **No personal health data in telemetry.** A tracing-layer attribute allowlist and a CI redaction test enforce it.
  - **Allowed:** UUIDs, node and facility ids, operation names, non-sensitive codes, durations and counts.
  - **Never allowed:** names, phone numbers, NINs, free text, or codes from sensitive ValueSets.
  - **In code** (`health-observability`): the export layer is the only exporter. A field is exported only if its name is on the allowlist and its value has that name's shape (a UUID for `*_id`, a short identifier with no run of seven or more digits, a route template, a code, or a number). Event messages, error values and `?Debug` values are never exported. Dropped fields are counted in `telemetry.redacted_fields`. HTTP spans record the route template, never the path or query (`health-http`). Codes from sensitive ValueSets are not detected by shape, so callers must not record them.
- **Indicators and alerts:**

  | Area | Indicators |
  |---|---|
  | Sync | Lag; outbox depth; open conflicts and rejections, and their age; `reseed_required` |
  | Storage | Backup freshness and verification; free disk; reindex jobs |
  | Platform | UPS state; clock skew against the clock floor; certificate and permission-snapshot expiry; software version against the allowed version |
  | Finance and stock | Pending-payment age; reconciliation exceptions; stock drift alerts |
  | Workflow | Stuck or overdue timers; queue wait times |

  Alerts go to central operations and are shown on the edge admin page.

## Disaster recovery (§92–94; ADR 0046)

| | Central | Edge / mode B desktop |
|---|---|---|
| Backup | Snapshot-consistent logical export **hourly** | Encrypted logical export hourly (ADR 0007) |
| Recovery path | Restore onto a **warm standby VM** at a second in-country site | Restore in place, or onto a cold spare |
| RPO | **1 h** for global records; facility records are re-pushed by edges after the epoch change (ADR 0019) | One backup interval (1 h) for unsynced work |
| RTO | **4 h** | < 1 h with a spare |
| Retention | 35 daily, 12 weekly, 12 monthly, plus an offsite in-country copy | 48 hourly, 14 daily |
| Keys | Backup master key under 3-of-5 Shamir custody, custodians named in the runbook | Per-node backup key, wrapped and escrowed in central KMS |
| Drills | Quarterly restore into staging, with integrity validation | Weekly automatic test restore into a temporary directory; restore drill at commissioning, then yearly |

- **After a central restore,** the sync servers rate-limit edge re-pushes.
- **Post-recovery reconciliation** compares the ledger digests and receipt counts for each node.
- **Enterprise or TiKV high availability** and the BSL "Database Service" question remain in the map's legal fog.
