# Client application: one React app across runtimes

Status: **accepted** (ADRs 0036, 0037, 0038). Produced while resolving the Wayfinder ticket [How does one React app adapt across central, LAN and Tauri runtimes?](https://github.com/carapai/people-centered-healthcare/issues/29). Background: [docs/research/frontend.md](../research/frontend.md). Spec: MASTER_PROMPT §12, §74–78, §98–101. MASTER_PROMPT §8 lists no architecture doc for the client, so this page is an addition. Related decisions:
- app delivery and device keys on the LAN: [lan-security.md](lan-security.md);
- sessions: [offline-auth.md](offline-auth.md);
- sync item states: [synchronization.md](synchronization.md);
- the desktop's roles and packaging: [desktop-client.md](desktop-client.md).

## Layers

```mermaid
flowchart TD
  UI[Feature modules<br/>routes, components, XState actors] --> P[Ports<br/>FhirClient · command ports per Bounded Context · EventSource]
  P --> T{Transport}
  T --> H[HttpTransport<br/>central: OIDC token · edge: DPoP token]
  T --> I[TauriIpcTransport<br/>invoke → tower oneshot → same Axum router]
  H --> C[(Central API)]
  H --> E[(Facility Edge API)]
  I --> R[(Desktop runtime<br/>embedded DB)]
```

### Ports

- **`FhirClient`:** read, vread, search (GET and POST), history, `$match`, dry-run `$extract`, `$populate`.
- **Command ports per Bounded Context,** which mirror the server use cases:

  | Bounded Context | Example command |
  |---|---|
  | Clinical Records | `completeQuestionnaireResponse` |
  | Clinical Operations | `recordSpecimenCollected` |
  | Finance | `recordPayment`, `evaluateGate` |
  | Supply Chain | `completeDispense`, `reserveStock` |

  The UI never assembles the FHIR writes for a business operation.
- **`EventSource`:** server-sent events from central or the edge, or Tauri events. Events carry only invalidation hints (resource type and id, or a worklist key), which invalidate Query keys.
- **In code** (`apps/client/src/platform/`):
  - **`fhir.ts`:** `fhirClient(transport, base)`. It has no write methods. A non-2xx response, or a body of another resource type, throws a `FhirError` carrying the status and the server's OperationOutcome. Its message names only the issue codes, because diagnostics can quote clinical data. `$match` posts `Parameters` (`resource`, `onlyCertainMatches`, `count`). `$extract` and `$populate` accept their output bare or inside `Parameters`.
  - **`operations.ts`:** `operationClient(transport).call(operationId, input)`. It calls an operation from `@health/api-client` and validates the response with `parseResponse`. The command ports of each Bounded Context build on it as their server use cases land. The first is `registries` (`platform/registries.ts`, #71), for `Patient/$register`. It loads the schemas (and Zod, about 84 KB) on the first call, not with the app shell.
  - **`events.ts`:** `sseEventSource(url, auth)` reads `{apiBase}/events` with `fetch`, because `EventSource` cannot send the Authorization and DPoP headers. Central serves the stream (`health_http::Events`, #218), filtered to the caller's facility and permissions.
    - One connection serves every subscriber. It reconnects with `Last-Event-ID`, starting at the server's `retry:` delay and doubling it up to 30 s.
    - It stops on a 4xx other than 401 or 429, or on a response that is not `text/event-stream` (for example, a web host's fallback page).
    - It drops hints that are not string identifiers.
  - **`query.ts`:** the query keys, `fhirKeys` and `worklistKey`. Everything for a resource type sits under `['fhir', type]`.
    - `readQuery`, `vreadQuery` (never stale), `searchQuery` and `historyQuery` are `queryOptions` over the `FhirClient`.
    - `connectQueryCache(platform, queryClient)` is called once in `main.tsx`. A resource hint invalidates that resource, its type's searches and histories, and system history. A type hint invalidates the whole type; a worklist hint invalidates its worklist.

### Transports

| §78 name | Ports + transport | Auth |
|---|---|---|
| `CentralHttpRepository` | `HttpTransport(central)` | Central OIDC session, tokens in memory |
| `FacilityLanHttpRepository` | `HttpTransport(edge)` | Edge DPoP tokens (ADR 0009/0010) |
| `TauriLocalRepository` | `TauriIpcTransport` | Local desktop session, carried over IPC |

- **One envelope everywhere:** method, path, query, headers subset and body. On the desktop, Rust dispatches IPC envelopes into the shared Axum router in-process (tower `oneshot`), with no loopback port.
  - A query or form parameter may repeat (`date=ge…&date=le…`). Search over POST sends a form body.
  - Over IPC, query parameters travel as ordered pairs and a form travels as its encoded text.
- **Decimals.** FHIR decimals and money are parsed with a lossless JSON parser in the transport.
- **Auth strategies** (`auth.ts`), chosen in `compose.ts`:
  - **Central:** `bearerAuth` sends `Authorization: Bearer` with the session's token.
  - **Edge:** `dpopAuth` sends `Authorization: DPoP` with an RFC 9449 proof for each request. The proof is ES256, signed with the device key, and carries `htm`, `htu` (no query or fragment), `iat`, a fresh `jti` and `ath`.
    - A `DPoP-Nonce` from the server is used from then on. A 401 `use_dpop_nonce` is retried once.
    - Enrollment ([#74](https://github.com/carapai/people-centered-healthcare/issues/74)) provides the non-extractable device key. Until then, edge requests carry no credentials.
  - Neither strategy sends anything without a token.

### Composition boundary

- **Detection:**
  - The Tauri build is a Vite build mode.
  - Browser builds fetch `runtime.json` from their host (central or edge), because the same bundle is served by both.
- **`Platform`.** `src/platform/compose.ts` builds one `Platform`:

  ```ts
  interface Platform {
    transport: Transport
    session: SessionPort
    events: EventSourcePort
    capabilities: {
      offlineCapable: boolean
      persistsLocally: boolean
      nativeKeystore: boolean
      printing: boolean
      deviceIntegration: boolean
      syncStatus: boolean
    }
  }
  ```

- **Distribution.** It is provided through React context and the TanStack Router context. Repositories are created from it once.
- **Lint rules:** `@tauri-apps/*` may only be imported in `src/platform/**`, and no runtime checks are allowed in feature code.
  - **Imports:** oxlint's `no-restricted-imports` (`apps/client/.oxlintrc.json`) enforces the `@tauri-apps/**` rule. We use oxlint because typescript-eslint has no TypeScript 7 support.
  - **Runtime checks and storage:** `apps/client/scripts/check-boundaries.ts` rejects runtime checks (`__TAURI`, `import.meta.env.MODE`, `platform.mode`) outside `src/platform/` and the status area. It also rejects browser storage APIs outside `src/platform/preferences.ts` (ADR 0037).
  - **Bundle scan:** `scripts/scan-bundle.ts` fails a web build that contains Tauri or devtools code, or Panda CSS without token variables.

### Types

- **FHIR R5 TypeScript types** (`@health/fhir-types`) are generated by `tools/codegen` from `health-fhir-validation`'s element table. That table is derived from `hl7.fhir.r5.core#5.0.0`, the same definitions the Rust model and validator use.
  - One interface per concrete resource and data type, with every element flattened (no inheritance). Backbone elements are nested interfaces named by path (`PatientContact`), reused for `contentReference`.
  - Choice elements become one optional property per type (`valueQuantity`). Primitives get their `_name` element.
  - JSON numbers are `LosslessNumber`, as the client parses them.
  - `FhirResource` is the union of resources, and `ResourceTypeMap` maps each type name to its interface.
- **Command DTOs:**
  1. the Rust handlers produce an OpenAPI document (`utoipa` 6 with `utoipa-axum`). `health_http::document` merges a runtime's paths with the system routes', and `central-server openapi` prints central's;
  2. `tools/codegen` generates Zod schemas, types (`z.infer`) and an `operations` table into `@health/api-client`. It supports the JSON Schema that utoipa emits for our DTOs and fails on anything else. JSON integers become safe JS numbers, and other numbers stay `LosslessNumber`;
  3. responses are validated with Zod at the transport boundary: `parseResponse(operation, status, body)` rejects undocumented statuses and mismatched bodies, reporting paths and codes, never values.
- **TypeScript 7 has no classic compiler API,** so the generators print text rather than use one. Common OpenAPI generators depend on that API.
- **CI** regenerates the OpenAPI document, the schemas and the FHIR types, and fails when they differ from the committed ones (frontend-ci's generated-types job).

## State ownership (ADR 0037)

| State | Owner | Rule |
|---|---|---|
| Server state, FHIR resources | TanStack Query | Repositories are `queryFn`/`mutationFn`; keys per resource type, id and search |
| URL state | TanStack Router | Search params validated with Zod |
| Field drafts | TanStack Form | Autosave goes through command ports (server-side drafts) |
| Processes: encounter flow, sign/finalize, session lock, sync and conflict views | XState v5 | Actors hold **ids only** and invoke commands |
| Widget state | Ark UI / `useState` | |
| Table view state | TanStack Table | Persisted through the URL or user preferences |

- **TanStack DB:** not used in v1.
- **TanStack Store:** not used directly.
- **TanStack Pacer:** wrapped in one debounce module.
- **TanStack Devtools:** dev only; CI checks that the production bundle contains none of it.

## What may persist where (§74–77)

| Mode | Clinical data | May persist |
|---|---|---|
| A: central web | In-memory Query cache only, `gcTime` 5 min | User preferences (localStorage, keyed by user). The service worker caches static assets only |
| D: LAN browser | Same as A | Preferences; the DPoP key (non-extractable, IndexedDB); the app-shell service worker |
| B/C Tauri | In the Rust-side embedded database only; the webview is in memory only | Preferences |

- **Never in a browser or webview:** clinical data in IndexedDB, OPFS or localStorage; API responses in a service worker; a mutation queue.
- **Drafts** in browser modes are server-side (e.g. QuestionnaireResponse `in-progress`).
- **Connection loss** in A and D: unsaved input stays in memory, a banner appears, and the user retries.

## Writes

- **Clinical and financial writes always wait for confirmation.** In Tauri, the local Rust commit is the confirmation. Only non-clinical preferences may be optimistic.
- **Tauri record badges** show the sync item state: saved locally, synced, under review, or rejected (synchronization.md).

## Sessions in the client

- **Tokens** are held in memory only.
- **Idle lock (15 min) and PIN user switch:**
  1. clear the Query cache;
  2. unmount patient views;
  3. show the lock screen or the next user.
- **Logout** clears everything except preferences and the device's DPoP key.
- **In code:** `memorySession()` keeps the token in a closure. `session.end('logout' | 'lock')` forgets the token, and `connectQueryCache` clears the Query cache on either.
- **Registry review** (`src/features/registry-review/`, #203): the review queue at `/registry/reviews`. Possible duplicates are compared side by side, field by field, and disagreeing values are shown with their trust and source. Decisions go to `/api/registry/reviews`.
- **Registration** (`src/features/registration/`, #71): reception's "did you mean" screen at `/registration`. It never creates a duplicate without the clerk's explicit confirmation, which the server enforces too. Candidates are chosen by keyboard in a DataTable.
- **End to end** (`e2e/`, #228): Playwright in Chromium (`bun run test:e2e`), nightly in `e2e.yml`. It drives the built client against a real central-server by keyboard: sign-in, registration with candidates and confirmation, the patient portal (phone sign-in through the development code outbox, self-registration, and a lookalike linked by a reviewer after an in-person check), caregiver delegation (a guardian's view of an adolescent, with and without her consent, and revoked), receiving stock with its barcode and cost, a ward requisition approved and sent, and axe on each of these screens. `HEALTH_DEV_BACKEND` makes `vite` and `vite preview` proxy `/auth`, `/api` and `/fhir` to a node.
- **FHIR base:** `runtime.json`'s optional `fhirBase` (central: `/fhir`) puts FHIR beside the API. Without it, FHIR is `{apiBase}/fhir`. The optional `ninSystem` names the national ID system.
- **Break-glass screens** (`src/features/break-glass/`, #218): the in-charge's alert list (`/break-glass/alerts`) and the privacy officer's review queue (`/break-glass/reviews`), as DataTables keyed by the worklists `/api/events` hints. An open list therefore refreshes when a grant is made or reviewed.
- **Sign-in** (`src/platform/sign-in.ts`, #188): the authorization code flow with PKCE S256 against the node's authorization server, configured by `runtime.json`'s `auth` (`issuer`, `clientId`, `redirectPath`).
  - **The redirect:** `start` leaves for `/authorize`, keeping the PKCE verifier and `state` in sessionStorage. That is the only storage besides preferences that `check-boundaries` allows, it holds no clinical data, and it is removed at the callback.
  - **The callback:** `/callback` checks `state` and `iss` (RFC 9207), then redeems the code.
  - **The patient portal** (#111): `runtime.json`'s `portal` names the portal's client and callback (`/portal/callback`). At central the platform gives it its own sign-in, session and API transport (`Platform.portal`), so patient and staff tokens never mix. For the portal's client, `/sign-in` asks for the phone (posted to `/otp`), then the code (posted to `/authorize`). `/portal` is the registration form and its status.
  - **The sign-in page:** `/sign-in` is the localized page the server sends the browser to (`HEALTH_AUTH_LOGIN_PAGE`). It posts the credentials straight to `/authorize` as a plain form, so the password never passes through application code, and comes back with `error=refused` on any refusal. Its optional facility code is needed only by users who work at more than one facility: without it, they come back with `error=facility` (ADR 0056).
  - **Tokens:** the access token lives in `memorySession` and the refresh token in a closure. A refresh runs a minute before expiry, and a failed refresh signs out locally.
  - **Sign-out:** revokes at `/revoke` and ends the session, which clears the Query cache.
  - **Scope:** central only for now. Edges sign in with DPoP-bound tokens once devices are enrolled (#74, #75).

## Forms (ADR 0038)

- **Renderer.** One `QuestionnaireRenderer` built on TanStack Form renders a Questionnaire with its Form Layout (configuration.md).
- **Live behaviour** is evaluated client-side with **fhirpath.js**: `enableWhenExpression`, `calculatedExpression`, answer constraints, population preview.
- **Server re-validation.** The server re-validates every submission, and extraction is Rust-only.
- **Conformance.** A shared FHIRPath conformance suite runs in CI against fhirpath.js and `octofhir-fhirpath`. Expressions in published Questionnaires must evaluate identically in both, and may use only constructs whose fixtures pass in both engines (ADR 0054); this is a publication gate. The suite includes decimal fixtures. The server recomputes `calculatedExpression` values on submit, and its value wins (ADR 0052).
- **Ordinary command forms** use TanStack Form with Zod schemas generated from the OpenAPI document.

## App shell

- **One route tree,** with lazily loaded feature modules per Bounded Context.
- **Gating.** Features are shown according to permission-snapshot claims, `Platform.capabilities` and configuration (active Healthcare Services and programs, ADR 0026).
- **Persistent status area (§101):**

  | Mode | Shows |
  |---|---|
  | A | Central reachable |
  | D | Edge reachable; edge ↔ central sync freshness |
  | Tauri | Local runtime; upstream reachable; outbox count; open conflicts and rejections; Authority Lease state |

## Serving and versions

- **The same build is served by:**
  - central (static hosting);
  - each edge, with assets embedded in the edge binary;
  - the Tauri bundle.

  LAN clients therefore always run their own edge's version.
- **Version check.** On load, the client checks its version against the API's supported range. On a mismatch it reloads (browser) or prompts the user to update (Tauri).

## Design system

- **Package:** `@health/design-system` (`packages/design-system/`), a Panda CSS 2 preset built with `panda lib`. The client loads it with `designSystem: '@health/design-system'` and imports `css` from `@health/design-system/css`; its `codegen` script runs `panda lib` first.
- **Tokens:** semantic tokens for light, dark and high-contrast, set by `data-theme` (`dark`, `high-contrast`; light when absent). **Density** modes (comfortable and compact) as a `[data-density=compact] &` condition. Both attributes work on `<html>` or on any subtree.
  - Comfortable controls are 44 px high (touch targets on tablets); compact ones are 32 px (1366×768 desktops), above the WCAG 2.5.8 minimum of 24 px.
  - Text pairs meet 4.5:1 and control borders 3:1 in each theme; axe checks colour contrast on the example page.
- **Components:**
  - our own slot recipes for Ark UI 5 components; Park UI is not used. The first set is Field, Select, Combobox, Dialog, Menu, Toast and Date Picker: each part is the Ark part with its slot class, and the composed parts (close buttons, indicators, the calendar) carry localized accessible names;
  - **DataTable** built on TanStack Table 9 and Virtual, with an ARIA grid, keyboard model and its own accessibility tests:
    - the [APG grid](https://www.w3.org/WAI/ARIA/apg/patterns/grid/) keyboard model: arrows, Home/End, Ctrl+Home/End, PageUp/PageDown, and Enter or Space on a header to sort. The grid has one tab stop, and the focused row stays rendered when scrolled away;
    - single-column sorting, cycling ascending, descending and off, with a status message; column visibility from a Columns menu;
    - the view is plain data that the caller owns. `viewToSearch` and `viewFromSearch` map it to `?sort=-age&hide=phone`, so routes keep it in the URL, and unknown or unsortable column ids are dropped;
    - manual NVDA checks are recorded in [accessibility-checks.md](../development/accessibility-checks.md);
  - icons from Lucide.
- **Accessibility:** WCAG 2.2 AA; axe in CI; manual NVDA testing on Windows.
- **Hardware targets:** low-end Windows PCs at 1366×768, and tablets with touch-target sizes.
- **i18n:** ICU messages, externalized from the first screen. English first.
  - The library is **react-intl** (FormatJS), whose messages are ICU MessageFormat. `I18nProvider` from the design system sets react-intl's locale and catalog and Ark's locale (date formats, reading direction).
  - Message ids are `ds.<component>.<name>` in the design system and `app.<feature>.<name>` in the client. The client merges both into one catalog per locale. Each message has a description for translators, and typed values.
- **Z-index tokens.** Panda 2's presets have none, so the design system defines `sticky`, `dropdown`, `overlay`, `modal` and `toast`. The bundle scan fails on a z-index that names an undefined token.
- **Ark UI declarations.** Ark 5.39's menu group declarations clash with `exactOptionalPropertyTypes` on `id`; `patches/@ark-ui%2Freact@5.39.3.patch` omits the HTML `id` there, so the app keeps full library checking. Drop the patch when Ark fixes it.
- **Pinned majors:** React 19, Router 1, Query 5, Form 1, Table 9, Virtual 3, XState 5, Ark UI 5, Panda 2, Zod 4, Vite 8, Bun 1.4, TypeScript 7. Prerelease majors are not adopted.

## Toolchain verified (ADR 0054)

These were checked by the spike [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38).
- **Bun.** Dev and build run under `bunx --bun vite`, and Node is not required.
- **TypeScript 7.** We pin `typescript@7.0.2`. Tooling config files are checked separately with `skipLibCheck`.
- **Panda 2.4.** The presets must be listed explicitly, and CI asserts that emitted CSS contains token variables.
- **React Compiler.** It costs about 15 ms per component file.
- **Tauri.** The `oneshot` IPC round trip into the Axum router matches HTTP.
- **fhirpath.js 5.2:**
  - It runs only over `lossless-json`-parsed resources with `preciseMath` and `keepDecimalTypes` on.
  - It passes 1,139 of the official suite's 1,307 tests.
  - Published Questionnaire expressions are limited to constructs that pass in both engines.
