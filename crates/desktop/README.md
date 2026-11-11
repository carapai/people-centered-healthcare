# crates/desktop

Desktop runtime crates for the Tauri client ([desktop-client.md](../../docs/architecture/desktop-client.md)).

- **`health-desktop-runtime`** (#132):
  - `ipc::dispatch`: the webview's `api_request` envelope into the shared Axum router through `tower` `oneshot`, with no socket (ADR 0036). Bodies cross as JSON text, so numbers stay lossless, and query parameters as ordered pairs. Only origin-relative paths are dispatched.
  - `data_dir::per_machine`: `%ProgramData%\<app>` or `/var/lib/<app>`, shared by every user's session on the PC.
  - `keystore`: the OS keystore port and the provisioning check that refuses to install without a working keystore. The `keyring` adapter comes with the Tauri binary.
  - The `ipc_server` example is the desktop node of the client's contract suite.

The Tauri binary (`apps/client/src-tauri`) composes them with the embedded store and the application crates.
