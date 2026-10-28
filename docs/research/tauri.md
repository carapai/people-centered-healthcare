# Research: Tauri v2

A condensed version of the research ticket [What does Tauri v2 offer for embedded local services and secure packaging?](https://github.com/carapai/people-centered-healthcare/issues/11). The full findings, with citations, are on the unmerged branch `research/tauri`. Decisions: [desktop-client.md](../architecture/desktop-client.md) (ADRs 0039–0041) and [client-application.md](../architecture/client-application.md) (ADR 0036).

## Versions and security

- **Current release:** `tauri` 2.12.5 and the CLI 2.12.4 (21 Oct). 2.0 went stable on 2024-10-02. A v3 alpha exists.
- **CVE-2026-42184:** origin confusion on Windows and Android. Fixed in 2.11.1.
- **GHSA-w28w-mhc8-qvjv:** cross-webview channel data theft. Fixed in 2.11.6, so **2.11.6 is the floor**.
- **Plugin requirement:** stronghold and the updater need Rust ≥ 1.90.

## IPC and process model

- **Commands:**
  - async commands run off the main thread;
  - **sync commands run on the main thread**;
  - JSON serialization is "slow for large data", so use raw `ipc::Request`/`Response`;
  - `Channel` gives ordered streaming;
  - events are fire-and-forget.
- **Core process.** Business logic, global state and database connections belong there.
- **Sidecars** are for external executables. They need per-target-triple copies, shell-plugin permissions and separate signing.
- **Loopback servers:** `tauri-plugin-localhost` carries a "considerable security risks" warning.
- **Capabilities** are per-window command allowlists with scopes. Commands must enforce scopes themselves, and by default all commands are callable from all windows.

## Secure storage

- **Stronghold:** an encrypted vault that needs a password-derived 32-byte key. Its last changelog entry was 2025-10.
- **No official keychain plugin.** The `keyring` crate covers Windows Credential Manager, macOS Keychain and Linux Secret Service. On Linux it needs GNOME Keyring or KWallet.

## Updates and signing

- **Updater:**
  - signatures are mandatory;
  - the key comes from `tauri signer generate`;
  - formats: AppImage, MSI/NSIS and `.app.tar.gz` (no deb/rpm);
  - endpoints are a static JSON file or a dynamic server.
  - Advisory GHSA-2rcp-jvr4-r259 means keeping `TAURI_SIGNING_*` out of `VITE_` env.
- **Windows:**
  - Authenticode reduces SmartScreen warnings; Smart App Control may block unsigned apps;
  - EV no longer gives instant reputation;
  - Azure Artifact Signing works via `signCommand`.
- **WebView2 install modes:** bootstrapper (needs internet), `offlineInstaller` (+127 MB) or `fixedVersion` (+180 MB, self-patched).
- **macOS:** a Developer ID ($99/yr), notarization, and Apple hardware.
- **Linux:** AppImage GPG signing, which AppImage itself doesn't verify.

## Webviews

| OS | Engine | Version source |
|---|---|---|
| Windows | WebView2 | Evergreen |
| macOS | WKWebView | Tied to the OS |
| Linux | WebKitGTK 4.1 | Tied to the distro |

- **Origins:** `http(s)://tauri.localhost` on Windows and Android; a custom scheme elsewhere.

## Embedded SurrealDB

- **Engines:** the Rust SDK embeds `kv-rocksdb` (C++, needs libclang) or `kv-surrealkv` (pure Rust).
- **No official Tauri example.** Third-party use exists (appdb, UMLBoard).
- **Unverified:** encryption at rest, multi-instance locking and size. These are checked by [Does embedded SurrealDB run reliably inside the Tauri process on facility PCs?](https://github.com/carapai/people-centered-healthcare/issues/39).
