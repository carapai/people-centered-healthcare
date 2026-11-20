# @health/client

The one React app served by central, by each edge, and inside the Tauri desktop (client-application.md, ADRs 0036–0038 and 0054).

```sh
bun install --frozen-lockfile     # from the repository root
bun run dev                       # Vite dev server under Bun
bun run check                     # codegen, typecheck, lint, build, scan
bun run test                      # platform-layer unit tests (Vitest, in Bun)
bun run test:contract             # FhirClient against a central and an edge FhirServer (needs cargo)
bun run test:e2e                  # Playwright against central-server (needs cargo and surreal)
bun run test:e2e:edge             # Playwright against facility-edge, built after the client so it embeds it
bun run build:tauri               # desktop webview bundle (--mode tauri)
```

## A seeded local backend

The e2e backend also runs by hand, for exploring the app with data:

```sh
cargo build -p central-server -p synthetic-data   # from the repository root
HEALTH_SEED=1 bun e2e/backend.ts                  # central on 18080, SurrealDB in memory on 18000
export HEALTH_DEV_BACKEND=http://127.0.0.1:18080        # in another shell: the app on 4173, as Playwright serves it
bun run build && bunx --bun vite preview --port 4173 --strictPort
```

- **Seed data:** `HEALTH_SEED=1` loads a small synthetic world from `tools/synthetic-data`: about 275 facilities with their stores and stock, 20 medicines, a price book, payers and contracts, and 300 patients by household. `HEALTH_SEED=<n>` loads `n` patients. It takes about 2.5 minutes, most of it stock. Everything is in memory, so it is seeded again each start.
- **Activity at KAM013** (`e2e/activity.ts`), through the same APIs the screens use:
  - **Stock:** 18 products in the product master, received into `Location/pharmacy` (the Stock screen's default store) and `Location/main-store`. Some pharmacy batches expire within three months. A ward's requisition waits for the pharmacy.
  - **Visits:** 18 patients checked in. 6 wait at triage, 6 are triaged and wait for consultation with their vital signs recorded, and 6 visits are completed.
  - **Appointments:** 11 booked over the next two weeks, and 5 patients' requests waiting in the clinician's inbox.
- **Admin:** sign in at KAM013 as `admin` / `harbour lantern meadow quartz` (`e2e/users.ts`), who holds every right: `*.cruds`, break-glass and every named permission (`ADMIN_PERMISSIONS`). A permission added to the code belongs in that list too. The e2e users are provisioned as well.
- **The Studio** (`/studio`) holds a draft of the OPD reference package (`ug.moh.opd`), written by `e2e-author`. It is a program package, so it needs two approvals. Sign in at KAM013 as, for each step (passwords in `e2e/users.ts`):

  | Step | Who |
  |---|---|
  | Submit | `e2e-author` |
  | Review | `e2e-studio` |
  | Approve (two) | `e2e-studio` and `e2e-approver` |
  | Publish, activate | `e2e-publisher` |

  The backend makes a throwaway publication-signing key with `openssl` for publishing. Without `openssl`, publishing answers that the key is missing.
- **A reload signs you out:** tokens live in memory only (ADR 0037). The next request's 401 sends you to sign in, and back to the page you were on after.
- **Beside another backend:** `HEALTH_E2E_CENTRAL_PORT`, `HEALTH_E2E_DB_PORT` and `HEALTH_DEV_OTP_OUTBOX` move its ports and one-time-code outbox, and `HEALTH_E2E_ORIGIN` the app's origin its sign-in redirects to (default `http://localhost:4173`).
- The e2e suite never sets `HEALTH_SEED`, so its runs start empty as before.

## Layout

- `src/platform/` is the only place that knows the runtime.
  - `compose.ts` builds the `Platform`: it fetches `runtime.json` in the browser, and the `tauri` Vite mode builds the desktop variant.
  - Transports: `http.ts` and `tauri.ts`. Auth strategies: `auth.ts` (central bearer; edge DPoP).
  - Ports: `fhir.ts` (`FhirClient`), `operations.ts` (OpenAPI operations, validated with Zod; command ports build on it), `events.ts` (server-sent invalidation hints).
  - `session.ts` keeps the token in memory. `query.ts` holds the Query keys and options, and `connectQueryCache` (invalidation hints, clearing on logout and lock).
  - JSON goes through `json.ts`, which uses `lossless-json`.
  - `preferences.ts` is the only browser storage, apart from:
    - the OAuth transaction (`sign-in.ts`), which holds the PKCE verifier and state across the redirect and nothing clinical;
    - the device's key pair on an edge (`device.ts`, #74; ADR 0009). It is an ECDSA P-256 pair whose private key is non-extractable, kept in IndexedDB as a `CryptoKey`.
  - `device.ts` holds that key. `compose.ts` loads it for edge mode, and `dpopAuth` signs every request with it: a proof alone before sign-in, which the edge's device gate requires, and bound to the token (`ath`) after.
  - `sign-in.ts` is the authorization code flow with PKCE, configured by `runtime.json`'s `auth`, at central and on an edge (#75). The `/sign-in` and `/callback` routes use it. `signInOnUnauthorized` wraps the API and FHIR transports: a 401 starts a sign-in that returns to the current page (#835). Pages that work signed out (`/sign-in`, `/callback`, `/enroll`, `/edge/…`, `/portal…`) and a locked session are left alone. On an edge, its `/token` and `/revoke` requests carry a DPoP proof by the device's key (`dpopProof`), so tokens are bound to the device and issued only to an enrolled one.
  - `shell.ts` registers the edge's app-shell service worker and runs the version check against `runtime.json`'s `build` (#76).
  - The build's ID is `HEALTH_BUILD_ID`, or the commit. `vite.config.ts` puts it in the bundle and in `dist/build.json`, which the edge embeds beside the bundle.
- `src/features/` holds one folder per Bounded Context. Feature code uses `usePlatform()` and the ports.
  - `lock/` (#75; offline-auth.md § Tokens and sessions), on an edge:
    - `LockGuard` wraps every page. After 15 minutes without a pointer, key, wheel or touch event, or when the edge refuses a refresh as `session_locked`, it calls `signIn.lock()`, which forgets the tokens and clears the query cache (`end('lock')`), and shows the lock screen instead of the page.
    - The lock screen's username and PIN either resume the session or switch the device to another user's live session there (`signIn.unlock`). Its fallback is a full password sign-in.
    - `SetPin` on the home page sets the user's PIN for this device (`signIn.setPin`).
  - `enrollment/` (#74) has two parts:
    - **`/enroll`:** the device makes its key, asks the edge, shows the pairing code, and polls until approved. It then confirms through the gate (`GET /api/device`), or offers to start over when revoked.
    - **`DevicesSection`:** on the edge status page, the pending requests (without their codes), approval by the typed code with a name and a location, and the enrolled devices, each revocable.
- `src/routes/` is the TanStack Router file routes. `routeTree.gen.ts` is generated by the Vite plugin and committed.
- `src/i18n/` holds the app's ICU messages (`app.*`) and the English catalog, merged with the design system's. Text shown to users goes through `useIntl()`.
- Styles come from `@health/design-system` (Panda `designSystem`); `codegen` builds it with `panda lib` first.
- `public/runtime.json` is the development default. Central and the edges serve their own copy.

## Checks

| Script | Fails when |
|---|---|
| `typecheck` | `tsc -b` over `tsconfig.json`, a solution file: TS 7 errors in the app (`tsconfig.app.json`, full lib checking), or in tool configs, scripts and e2e (`tsconfig.node.json`, Bun's types, with `skipLibCheck`) |
| `lint` | oxlint errors, including `@tauri-apps/**` imported outside `src/platform/**`; runtime checks or browser storage outside the allowlist (`scripts/check-boundaries.ts`) |
| `test` | A platform-layer unit test fails (`test/`) |
| `test:contract` | The `FhirClient` suite fails against either the central or the edge node (`test/contract/`; client-application.md § Ports) |
| `test:contract` (desktop) | The same suite fails over IPC, through `ipcTransport` into `health-desktop-runtime` |
| `scan` | The web bundle contains Tauri or devtools code, or Panda CSS has no token variables, has raw shorthands, or names an undefined z-index token |

## The desktop (Tauri)

`src-tauri/` is the desktop binary (`health-desktop`), its own Cargo workspace (desktop-client.md § In code). Build the web app first (`bun run build`), since the webview's assets are embedded at compile time. Then, in `src-tauri/`, run `cargo build` or `cargo run`, with `HEALTH_DESKTOP_DATA_DIR` set to a writable directory in development. It needs WebKitGTK 4.1 on Linux and WebView2 on Windows. The client starts only on a machine whose system and data volumes are encrypted (BitLocker or LUKS). On a development machine without it, or on macOS, which is not probed, set `HEALTH_DESKTOP_ENCRYPTION=warn`:

```sh
HEALTH_DESKTOP_DATA_DIR=~/health-desktop-data HEALTH_DESKTOP_ENCRYPTION=warn cargo run
```

A standalone desktop (mode B) is provisioned once, with a one-time code from central: `cargo run -- provision --code XXXX-XXXX --central https://central… --root root.pem` (desktop-client.md § Provisioning). Once provisioned it syncs with central; a first install the link cannot carry is seeded from removable media with the client closed: `cargo run -- seed import KLA001.seed`, with `KLA001.seed.jws` beside it (`central-server seed export`).
