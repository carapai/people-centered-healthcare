# Tauri v2: embedded local services and secure packaging

Research ticket: #11. Spec context: `MASTER_PROMPT.md` §31 (desktop crates), §75 (Mode B, standalone offline Tauri), §77 (Mode D, LAN client), §78 (shared React app), §114 (edge and desktop builds). Researched 2026-10-27.

## Summary

Stable Tauri is in the **2.12.x** line. The `tauri` crate v2.12.5 was released on 21 Oct, and the CLI is at 2.12.4. Tauri v2 supports the §75 architecture (React → Tauri commands → local Rust services → embedded DB) directly:

- **Run local services in-process.** The local services should be ordinary Rust crates linked into the Tauri core process and held in managed state. Tauri's own guidance is to put business logic, global state and database connections in the core process. Sidecars exist for bundling *external* executables. They add per-target-triple binaries, shell-plugin permissions and separate signing, so they are the wrong tool for our own Rust services.
- **IPC is enough for clinical workloads.** Commands are async and JSON-serialized by default. v2 added raw byte payloads (`tauri::ipc::Request` / `Response`) and `Channel` for ordered streaming. The IPC runs over custom protocols instead of v1's JSON-only bridge.
- **Use Tauri ≥ 2.11.6 for security fixes.** The capability/permission/scope system gives each window per-command allowlists. Two 2026 advisories (origin confusion, CVE-2026-42184, and cross-webview channel data theft, GHSA-w28w-mhc8-qvjv) are patched only in **≥ 2.11.6**, so this is the floor.
- **Secure storage needs a decision.** The official `tauri-plugin-stronghold` is an encrypted file vault that needs a password or key. There is **no official OS-keychain plugin**. Keychain or Credential Manager access means using the `keyring` crate directly or a small community plugin.
- **Updates must be signed with Tauri's own key.** This cannot be disabled, and the formats are AppImage, MSI/NSIS and `.app.tar.gz`; deb/rpm are not covered. OS signing is a separate job: Windows Authenticode (OV/EV or Azure Artifact Signing), macOS Developer ID plus notarization ($99/yr Apple account, needs a Mac), and optional GPG signing for Linux AppImages.
- **One React bundle needs a lowest-common-denominator baseline.** The bundle runs on three different engines: WebView2/Chromium (evergreen), WKWebView (tied to the macOS version) and WebKitGTK 4.1 (tied to the distro). The page origin differs per OS: `http(s)://tauri.localhost` on Windows/Android vs a custom scheme elsewhere. This affects CORS when a Tauri client calls a Facility Edge API.
- **Embedded SurrealDB is technically straightforward but unproven here.** The SurrealDB Rust SDK (latest server release v3.3.4, 19 Oct) supports in-process `kv-surrealkv` (pure Rust) and `kv-rocksdb` (C++, needs libclang). There is no official Tauri+SurrealDB example, so §75's "only after validation" gate stands and needs a spike.

## Findings

### 1. Current stable version

- `tauri` crate v2.12.5 (21 Oct) and `@tauri-apps/cli` / `tauri-cli` v2.12.4. The releases page shows "21 Oct" with no year; given today's date, it is presumably 2026 (year **unverified** from the page). Source: https://github.com/tauri-apps/tauri/releases
- Tauri 2.0 went stable on 2 Oct 2024. Source: https://v2.tauri.app/blog/tauri-20/
- A v3 alpha exists (3.0.0-alpha.3). Its notes list breaking plugin and window API changes. Source: https://newreleases.io/project/github/tauri-apps/tauri/release/tauri-v3.0.0-alpha.3 (secondary mirror of GitHub release notes; v3 timeline **unverified**).
- Plugins (stronghold, updater) require Rust ≥ 1.90. Sources: https://v2.tauri.app/plugin/stronghold/ , https://v2.tauri.app/plugin/updater/

### 2. Command / IPC model and throughput

- **Commands:**
  - Defined with `#[tauri::command]`, registered via `generate_handler!` and called with `invoke()` from JS.
  - Arguments arrive as a JSON object with camelCase keys by default. They must be `Deserialize`, and returns must be `Serialize`.
  - A `Result` error rejects the JS promise.
  - Source: https://v2.tauri.app/develop/calling-rust/
- **Threading:**
  - `async` commands run on a separate async task.
  - **Non-async commands run on the main thread** unless marked `#[tauri::command(async)]`. A blocking DB call in a sync command freezes the UI.
  - Borrowed args (`&str`, `State<'_, T>`) are awkward in async commands, so use owned types.
  - Source: https://v2.tauri.app/develop/calling-rust/
- **Raw payloads:**
  - JSON serialization is "slow for large data".
  - `tauri::ipc::Response` returns raw bytes.
  - `tauri::ipc::Request` receives `InvokeBody::Raw` when JS sends an `ArrayBuffer`/`Uint8Array`, with access to headers.
  - Source: https://v2.tauri.app/develop/calling-rust/
- **v2 IPC rewrite:**
  - v1 JSON-serialized every payload, which caused noticeable overhead above a few KB.
  - v2 uses custom protocols. Raw requests allow any serialization format (protobuf, Avro, etc.).
  - On Windows and Android the custom protocol uses the `http` scheme.
  - Source: https://v2.tauri.app/blog/tauri-20/
- **Channels:**
  - `Channel` is the recommended way to stream ordered data to the frontend (e.g. chunked files).
  - Events are fire-and-forget, JSON-only and not type-safe. They can arrive out of order under async listeners.
  - Source: https://v2.tauri.app/develop/calling-rust/
- **IPC conceptual model:** asynchronous message passing in a JSON-RPC-like protocol. The core can intercept, filter or reject every message. Source: https://v2.tauri.app/concept/inter-process-communication/
- **Throughput figures:** Tauri publishes no official MB/s or latency benchmarks for v2 IPC, so concrete numbers are **unverified**. We need our own benchmark, e.g. a FHIR Bundle of N resources via JSON vs raw vs channel.

### 3. In-process Rust services vs sidecars

- **Core process:**
  - It is the only component with full OS access and routes all IPC.
  - It "should hold global state such as settings and database connections" so that sensitive data stays away from the frontend.
  - Business logic should live there "as much as possible".
  - Source: https://v2.tauri.app/concept/process-model/
- **Sidecars:**
  - External executables listed in `bundle.externalBin`.
  - Each needs a `-$TARGET_TRIPLE`-suffixed copy.
  - Running one requires the shell plugin plus a `shell:allow-execute` / `shell:allow-spawn` capability with argument validators.
  - Spawned from Rust with `app.shell().sidecar(..)`; communication is over stdin/stdout.
  - The sidecar page does not cover signing sidecar binaries (**unverified** how notarization treats them; macOS generally requires every nested executable to be signed).
  - Source: https://v2.tauri.app/develop/sidecar/
- **Loopback HTTP server alternative:**
  - An alternative is for the core to start an Axum server on loopback and have the webview `fetch` it. This is not a Tauri feature; it is just a TCP listener in the process.
  - The related `tauri-plugin-localhost`, which serves *assets* from localhost, carries the warning "This plugin brings considerable security risks and you should only use it if you know what you are doing." Source: https://v2.tauri.app/plugin/localhost/
  - By analogy, an unauthenticated loopback API is reachable by any local process, while Tauri commands are gated by capabilities. This risk assessment is our own inference, not a Tauri statement.

### 4. Capabilities / permissions / scopes

- **Model:** v2 replaces the v1 allowlist with three concepts:
  - **Permissions**: on/off toggles per command.
  - **Scopes**: parameter validation.
  - **Capabilities**: attach permissions and scopes to windows and webviews.
  - Core checks whether an invoke is allowed, but **the command implementation must enforce the scope**.
  - Source: https://v2.tauri.app/blog/tauri-20/
- **Capability files:**
  - Live in `src-tauri/capabilities/*.json|toml` and target window labels.
  - They can be restricted per platform.
  - A window in several capabilities gets the merged permissions.
  - By default, all registered app commands are callable from all windows; restrict this with `AppManifest::commands`.
  - `remote.urls` can grant remote origins access. On Linux and Android, Tauri cannot distinguish iframe requests from the window itself.
  - Capabilities do not protect against insecure Rust code.
  - Source: https://v2.tauri.app/security/capabilities/
- **Isolation pattern:**
  - Optional. A sandboxed iframe intercepts and validates every IPC message, then AES-GCM-encrypts it with per-run keys.
  - Aimed at compromised dependencies.
  - Has a small performance cost, and ES modules won't load inside the isolation app.
  - Source: https://v2.tauri.app/concept/inter-process-communication/isolation/
- **2026 advisories directly relevant to us:**
  - **CVE-2026-42184 / GHSA-7gmj-67g7-phm9** (Moderate). On Windows and Android, `is_local_url()` checked only the first subdomain label, so a remote page could invoke local-only IPC. Affects 2.0 – 2.11.0; fixed in **2.11.1**. Source: https://github.com/tauri-apps/tauri/security/advisories/GHSA-7gmj-67g7-phm9
  - **GHSA-w28w-mhc8-qvjv** (High, CVSS 7.6). Large channel responses sat in an app-wide queue with sequential IDs, so a compromised webview could read another webview's payloads. Affects 2.0.0 – 2.11.5; fixed in **2.11.6**, with no workaround. Source: https://github.com/tauri-apps/tauri/security/advisories/GHSA-w28w-mhc8-qvjv
  - Advisory index: https://github.com/tauri-apps/tauri/security/advisories

### 5. Secure storage

- **Stronghold:**
  - `tauri-plugin-stronghold` uses the IOTA Stronghold engine, an encrypted on-disk vault.
  - Supports Windows, Linux, macOS, Android and iOS.
  - It must be initialized with a password-hash function: the built-in `Builder::with_argon2(salt_path)` or a custom function returning exactly 32 bytes.
  - Source: https://v2.tauri.app/plugin/stronghold/
  - Its README shows no deprecation notice. Source: https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/stronghold
  - Its changelog's last entry is 2.3.1 (27 Oct 2025, "no user facing changes"), so development activity looks low. Source: https://v2.tauri.app/release/stronghold/all-versions/
  - Whether Stronghold survives into v3 is **unverified**.
- **OS keychain:**
  - There is no official Tauri plugin.
  - The `keyring` crate covers macOS/iOS Keychain, Windows Credential Manager, and Linux Secret Service or kernel keyutils. On Linux it needs a running Secret Service such as GNOME Keyring or KWallet. Source: https://docs.rs/crate/keyring/4.1.3
  - A community wrapper exists, https://github.com/HuakunShen/tauri-plugin-keyring, with small adoption; its maintenance status is **unverified**.
  - Because services are in-process Rust, we can call `keyring` directly from `health-desktop-auth` and need no plugin. In particular, there is no need to expose secrets to JS at all.

### 6. Updater and code signing

- **Updater plugin (`tauri-plugin-updater`):**
  - Signatures are mandatory ("This cannot be disabled").
  - Keys come from `tauri signer generate`. The private key is passed via `TAURI_SIGNING_PRIVATE_KEY` (plus optional password); `.env` files are not supported.
  - Set `createUpdaterArtifacts: true` for v2-format artifacts.
  - Endpoints can be a static JSON file (version, plus url and signature per platform) or a dynamic server (204 means no update).
  - Formats: Linux **AppImage** only (deb/rpm not mentioned); macOS `.app.tar.gz`; Windows MSI or NSIS.
  - Windows `installMode` is `passive`, `basicUi` or `quiet`.
  - Source: https://v2.tauri.app/plugin/updater/
  - The page does not name the signature algorithm (commonly reported as minisign; **unverified** from primary docs).
  - Advisory GHSA-2rcp-jvr4-r259 warned that updater private keys could leak via Vite env variables, so keep `TAURI_SIGNING_*` out of `VITE_`-exposed env. Source: https://github.com/tauri-apps/tauri/security/advisories
- **Windows (Authenticode):**
  - Signing reduces SmartScreen warnings. Unsigned apps can be blocked entirely by Windows 11 Smart App Control.
  - Since 2024, EV certificates no longer give instant SmartScreen reputation.
  - Options: OV certificate via `certificateThumbprint`/`timestampUrl`; Azure Key Vault via `relic` in `signCommand`; or **Azure Artifact Signing** (formerly Trusted Signing) via `signCommand`.
  - Cross-compiling installers from non-Windows hosts requires a custom `signCommand`.
  - Source: https://v2.tauri.app/distribute/sign/windows/
- **Windows installers:**
  - MSI is built with WiX v3, only on Windows, and needs the VBSCRIPT feature (which Microsoft is deprecating). NSIS cross-compiling is "last resort".
  - WebView2 install modes:

    | Mode | Size | Needs internet |
    |---|---|---|
    | `downloadBootstrapper` | 0 MB | yes |
    | `embedBootstrapper` | ~1.8 MB | yes |
    | `offlineInstaller` | ~127 MB | no |
    | `fixedVersion` | ~180 MB | no; we patch it ourselves |
  - Sources: https://v2.tauri.app/distribute/windows-installer/ , https://v2.tauri.app/start/prerequisites/
- **macOS:**
  - Needs a paid Apple Developer account ($99/yr) and an Apple device to sign.
  - Outside the App Store, use a Developer ID Application certificate, which only the Account Holder can create. Notarization is required.
  - CI setup: `APPLE_CERTIFICATE`/`APPLE_CERTIFICATE_PASSWORD`, plus either the App Store Connect API key vars or `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID`.
  - Ad-hoc signing (`"-"`) still needs a user approval step.
  - Source: https://v2.tauri.app/distribute/sign/macos/
- **Linux:**
  - Signing is optional. AppImage GPG signing uses `SIGN=1`, `SIGN_KEY`, `APPIMAGETOOL_SIGN_PASSPHRASE`, and `APPIMAGETOOL_FORCE_SIGN=1` to fail the build if signing fails.
  - AppImage does **not** verify signatures itself.
  - deb/rpm signing is not covered by Tauri docs; it is distro/repo tooling and **unverified** here.
  - Source: https://v2.tauri.app/distribute/sign/linux/
- **Bundle targets:** `deb`, `rpm`, `appimage`, `msi`, `nsis`, `app`, `dmg`. Source: https://v2.tauri.app/reference/config/

### 7. WebView engines and the shared React bundle

- **Engines per OS:**

  | OS | Engine | How the version is set |
  |---|---|---|
  | Windows | WebView2 (Chromium) | Self-updating. Preinstalled on Win 11 and on Win 10 1803+ |
  | macOS | WKWebView | Tied to the OS version; unsupported macOS versions get no WebKit updates |
  | Linux | WebKitGTK | Whatever version the distro packages |

  Source: https://v2.tauri.app/reference/webview-versions/
- **Minimums:**
  - Linux needs WebKitGTK **4.1** (`libwebkit2gtk-4.1-dev`).
  - macOS needs **10.15+**.
  - Windows needs **7+**, though Win7 needs care in installer mode.
  - Source: https://v2.tauri.app/start/prerequisites/
- **Process model:** webview libraries are linked at runtime rather than bundled. This keeps apps small but "platform differences matter". Source: https://v2.tauri.app/concept/process-model/
- **Origins:**
  - On Windows and Android, custom protocols map to `http://<scheme>.localhost`. `useHttpsScheme` (added in 2.1.0) switches this to `https://`.
  - On macOS and Linux the app is served from the custom scheme itself.
  - Sources: https://v2.tauri.app/blog/tauri-20/ , https://v2.tauri.app/release/tauri-utils/v2.1.0/
  - The exact `Origin` header string sent on macOS/Linux is **unverified**; test it in a build.
- **CSP:**
  - `app.security.csp` sets the policy, and Tauri modifies it for assets unless `dangerousDisableAssetCspModification` is set.
  - `app.security.pattern` selects brownfield (default) or isolation.
  - Source: https://v2.tauri.app/reference/config/

### 8. Embedded SurrealDB inside the Tauri process

- **Engines:**
  - The SurrealDB Rust SDK embeds in-process with `kv-mem`, `kv-rocksdb` or `kv-surrealkv`.
  - RocksDB is C++ and needs a C++ compiler and libclang at build time. SurrealKV is pure Rust and needs neither.
  - `engine::any::connect` switches between embedded and remote engines by connection string.
  - Records need to derive `SurrealValue` (serde alone does not compile with `.content()`).
  - Source: https://surrealdb.com/docs/sdk/rust/embedding
- **Version:** the latest SurrealDB release is v3.3.4 (19 Oct). Source: https://github.com/surrealdb/surrealdb/releases
- **SurrealKV:** an embedded ACID key-value engine with built-in versioning. Source: https://surrealdb.com/docs/surrealkv
- **Desktop guidance:** SurrealDB's JS and Python embedding guides recommend SurrealKV for desktop persistence. Sources: https://surrealdb.com/docs/sdk/javascript/embedding , https://surrealdb.com/docs/sdk/python/embedding
- **Tauri examples:**
  - There is no official Tauri+SurrealDB example.
  - Third-party evidence: the `appdb` crate opens embedded SurrealKV for Tauri-style apps (https://docs.rs/crate/appdb/latest), and a write-up describes UMLBoard on Tauri 2 with RocksDB-backed SurrealDB (https://dev.to/pgenfer/moving-from-electron-to-tauri-2-1jbl).
  - These are not primary sources, so production fitness is **unverified**.
- **Unverified pending a spike:** encryption-at-rest for embedded SurrealKV/RocksDB, file-locking behavior when two app instances start, and binary-size and compile-time impact.

## Implications for the decision

### Options

1. **In-process services with Tauri commands (recommended baseline).**
   - The `health-desktop-*` crates (§31) are linked into the Tauri core and exposed as thin `#[tauri::command]`s.
   - The `TauriLocalRepository` adapter (§78) wraps `invoke`. Commands are always `async`; large FHIR payloads go as raw bytes and sync progress or streams use `Channel`.
   - Pros: no extra process to sign, no open port, and capability-gated.
   - Cons: a second transport besides HTTP, so the domain-to-DTO mapping must be shared to avoid divergence.
2. **In-process Axum on loopback, with the webview calling HTTP.**
   - Maximizes reuse with `CentralHttpRepository` and `FacilityLanHttpRepository`, since it is the same API surface as the Facility Edge.
   - Costs: an authenticated loopback port (any local process can connect), CORS handling for the per-OS origin, and bypassing the capability system.
   - Could be acceptable if guarded by a per-launch random token passed via a command. This is our design, not a Tauri feature.
3. **Sidecar (separate edge binary bundled with the app).**
   - Fits only if one artifact must serve both standalone and Facility Edge roles.
   - Adds per-triple binaries, shell-plugin permissions and nested signing/notarization.
   - Edge should remain separately built (§114), so this is not recommended for Mode B.

### Risks

- Tauri version floor ≥ **2.11.6** because of the channel and origin advisories. Pin and track `tauri` advisories in CI (§115).
- WebKitGTK and old-macOS WKWebView lag Chromium. The React/Vite build target and polyfills must be set for the oldest supported WebKit, and the UI needs e2e runs on all three engines.
- Offline provisioning on Windows calls for `offlineInstaller` (+127 MB) or `fixedVersion` (+180 MB, self-patched) for WebView2 if target machines may lack it (Win 7/8, stripped images).
- Signing costs and logistics:
  - Apple Developer account and a Mac runner for notarization.
  - A Windows certificate or Azure Artifact Signing subscription.
  - Custody of the updater signing key: losing it strands installed clients.
  - Per §115, we must not claim releases are signed until this is configured.
- The Linux updater only supports AppImage. deb/rpm installs need distro repo updates instead.
- Secure storage:
  - Linux keyring needs a Secret Service, which may be absent on kiosk or minimal installs.
  - Stronghold needs a password or key and looks low-activity.
  - We need a fallback chain: OS keyring, then a Stronghold or app-encrypted file keyed from user credentials.
- Embedded SurrealDB in the desktop is unvalidated (§75). The open questions are encryption at rest, crash recovery, multi-instance locking and upgrade migrations.

### Open questions

- Which OS versions and distros are we committing to? This determines the WebKit baseline and Win7 support.
- Is option 1 (commands) or option 2 (loopback HTTP) the `TauriLocalRepository` transport? A benchmark of a realistic FHIR Bundle over JSON vs raw vs channel should inform this.
- SurrealKV vs RocksDB for embedded storage, and how at-rest encryption is achieved (OS disk encryption, app-level field encryption, or engine support, which is **unverified**).
- Who holds the Apple, Windows and updater signing keys, and where (HSM / Azure Key Vault)?
- Do LAN Tauri clients (§77) talk to the Facility Edge directly from the webview (CORS on per-OS origins) or via Rust-side HTTP in the core? The latter avoids CORS and keeps tokens out of JS.
