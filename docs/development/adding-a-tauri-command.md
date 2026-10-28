# Adding a Tauri command

> Status: planned layout. No code exists yet; the IPC dispatch command and `src/platform/` are created by the scaffold.

## When to use

Read this before writing any `#[tauri::command]`. In this architecture, **business operations are not Tauri commands.** The webview sends the same request envelope it would send over HTTP through `TauriIpcTransport`, and the desktop runtime dispatches it into the **same Axum router** in-process (tower `oneshot`). Hand-written per-use-case commands were rejected because a second API surface would drift (ADR 0036).

So, in almost every case, "adding a Tauri command" means **adding a route**: follow [adding-an-api.md](adding-an-api.md) and the operation becomes available on the desktop automatically.

## When a native command is justified

Only for things the shared router cannot do, i.e. OS integration behind `Platform.capabilities` (for example printing or device integration). Anything touching clinical data or business rules still goes through the router.

## Steps (native command only)

1. Confirm the capability is not a business operation and is listed (or should be listed) in `Platform.capabilities`.
2. Implement it in the desktop runtime (`health-desktop-runtime`, or `apps/client/src-tauri`), as an **`async`** command. Sync commands would run on the main thread.
3. Large payloads use raw `ipc::Request` / `Response` bodies; progress streams use `Channel`.
4. Enforce any scope inside the command; capabilities do not do it for you.
5. Add the command to the explicit allowlist (`AppManifest::commands`) for the single main window. No remote URL gets capabilities.
6. Call it from the client **only** inside `apps/client/src/platform/**`, behind a port; feature code never imports `@tauri-apps/*`.
7. Never return secrets to JavaScript. Keys and tokens stay in the OS keystore via `keyring`, used from Rust (ADR 0040).

## Expected files

- `apps/client/src-tauri/…` or `crates/desktop/health-desktop-runtime/src/…` (command)
- Tauri capability / manifest files (layout to be fixed by the scaffold)
- `apps/client/src/platform/…` (the adapter that invokes it)

## Contracts

- The IPC envelope (method, path, query, header subset, body), identical to HTTP.
- `Platform` and its `capabilities` flags.
- The command allowlist.

## Tests

- **API contract:** HTTP and IPC transports return identical envelopes.
- **Frontend:** lint rule confining `@tauri-apps/*` to `src/platform/`; production web bundle scan finds no `@tauri-apps`.
- **End-to-end:** Tauri via `tauri-driver` on Windows and Linux (nightly).

## Governing decisions

[ADR 0036](../adr/0036-ports-over-swappable-transport-with-tauri-ipc-into-the-shared-router.md), [ADR 0039](../adr/0039-one-desktop-binary-two-roles-with-rust-side-failover.md), [ADR 0040](../adr/0040-desktop-secrets-in-os-keystore-and-disk-encryption-check.md), [desktop-client.md](../architecture/desktop-client.md), [client-application.md](../architecture/client-application.md), [research/tauri.md](../research/tauri.md).
