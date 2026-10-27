# Research: frontend stack stability (issue #12)

Question: which TanStack, XState, Ark UI and Panda CSS pieces are stable enough to depend on?
Scope: MASTER_PROMPT.md §12 (frontend stack), §78 (one shared React app with platform adapters), Part 17 §§98–101 (Ark UI + Panda design system, TanStack Form engine, offline/sync status UX).
Research date: 2026-10-27. Version data comes from the npm registry (`npm view <pkg> version dist-tags time`) on that date unless a URL is given.

## Summary

- **Stable now (≥1.0, depend on it):** React 19.3, React Compiler 1.0, TanStack Router 1.x, Query 5.x, Form 1.x, Table 9.x (v9 went GA on 2026-08-04), Virtual 3.x, XState 5.x with `@xstate/react` 6.x, Ark UI 5.x, Panda CSS 2.x (Rust/Oxc engine since 2026-09-29), Zod 4.x, Vite 8.x (Rolldown), Bun 1.4, Lucide 1.x.
- **Still pre-1.0 (allowed only where the spec says "where justified"):** TanStack DB (`@tanstack/db` 0.15, `@tanstack/react-db` 0.5), Store 0.11, Pacer 0.23/0.24 (docs say **beta**), Devtools 0.10 (docs say **alpha**). The DB satellites `@tanstack/query-db-collection` 1.4 and `@tanstack/offline-transactions` 1.1 are 1.x, but they sit on a 0.x core.
- **Next majors already in prerelease:** Form 2.0 alpha, XState 6.0 alpha, Ark UI 6.0 `next`, Store 1.0 alpha. Pin to the current majors and plan an upgrade window. Don't adopt the prereleases.
- **Ark UI has no data table or data grid.** Tables come from TanStack Table (headless) plus TanStack Virtual, styled with Panda. Ark UI does cover date picker, date input, combobox, select, listbox, dialog, menu, toast, tree view, number input and tags input.
- **State ownership:** Query owns server state. DB is optional and wraps Query; it doesn't replace it. XState owns workflow and process state. Form owns draft field state. Router owns URL state. Store is mostly an internal dependency (Form already depends on it), so app code shouldn't add its own Store layer.

## Version and maturity table

| Package | Latest stable | First release of current major | ≥1.0? | Status / next major | Source |
|---|---|---|---|---|---|
| react / react-dom | 19.3.1 | 19.0.0: 2024-12-05; 19.3.0: 2026-09-09 | yes | stable | npm `react` |
| babel-plugin-react-compiler | 1.0.0 | 2025-10-07 | yes | stable 1.0 | npm; setup: https://react.dev/learn/react-compiler/installation |
| @tanstack/react-router (+ router-plugin) | 1.170.46 (plugin 1.168.48) | 1.0.0: 2023-12-23 | yes | stable | npm; https://tanstack.com/router/latest/docs/framework/react/overview |
| @tanstack/react-query | 5.105.0 | 5.0.0: 2023-10-17 | yes | stable | npm |
| @tanstack/react-form | 1.33.5 | 1.0.0: 2025-03-02 | yes | **2.0.0-alpha.3** published 2026-10-23 | npm; https://tanstack.com/form/latest/docs/framework/react/guides/validation |
| @tanstack/react-table | 9.3.1 | 9.0.0: 2026-08-04 | yes | stable v9 (v8 was last 8.21.3) | npm; https://tanstack.com/table/latest/docs/overview |
| @tanstack/react-virtual | 3.14.23 | 3.0.0: 2023-12-01 | yes | stable | npm |
| @tanstack/db / react-db | 0.15.2 / 0.5.12 | 0.1.0: 2025-07-29 | **no** | pre-1.0; docs flag APIs "deprecated and will be removed in v1.0" | npm; https://tanstack.com/db/latest/docs/collections/query-collection |
| @tanstack/query-db-collection | 1.4.8 | 1.0.0: 2025-11-12 | yes, but depends on 0.x core | peer `@tanstack/query-core ^5.90.15` | npm |
| @tanstack/offline-transactions | 1.1.4 | n/a | yes, but depends on 0.x core | | npm; https://tanstack.com/db/latest/docs/guides/offline-transactions |
| @tanstack/tauri-db-sqlite-persistence | 0.2.37 | n/a | **no** | early | npm |
| @tanstack/store / react-store | 0.11.2 | 0.11.0: 2026-04-17 | **no** | **1.0.0-alpha.0** published 2026-10-10 | npm; https://tanstack.com/store/latest/docs/overview |
| @tanstack/pacer / react-pacer | 0.23.1 / 0.24.1 | 0.1.0: 2025-04-07 | **no** | docs: "currently in beta and its API is still subject to change" | https://tanstack.com/pacer/latest/docs/overview |
| @tanstack/react-devtools (+ devtools-vite) | 0.10.13 (vite 0.8.5) | 0.1.0: 2025-08-01 | **no** | docs: "currently in alpha and its API is subject to change" | https://tanstack.com/devtools/latest/docs/overview |
| per-library devtools | query-devtools 5.105.0, router-devtools 1.167.2, form-devtools 0.2.34, pacer-devtools 0.9.1, table-devtools 9.3.1 | | mixed | | npm |
| xstate | 5.33.3 | 5.0.0: 2023-12-01 | yes | **6.0.0-alpha.70** published 2026-10-19 | npm; https://stately.ai/docs/xstate; GitHub releases statelyai/xstate |
| @xstate/react | 6.1.0 | 6.0.0: 2025-06-19 | yes | peer `xstate ^5.28.0` | npm |
| @xstate/store | 4.3.0 | | yes | 5.0 alpha | npm |
| @ark-ui/react | 5.39.3 | 5.0.0: 2025-03-06 | yes | **6.0.0-next.0** published 2026-10-06; built on `@zag-js/*` 1.45.0 | npm |
| @pandacss/dev (+ @pandacss/vite, @pandacss/bun) | 2.4.0 | 2.0.0: 2026-09-29 | yes | v2 is ESM-only and needs Node ≥22 | https://github.com/chakra-ui/panda/releases/tag/%40pandacss/dev%402.0.0 |
| zod | 4.6.7 | 4.0.0: 2025-07-09 | yes | "Zod 4 is now stable!" | https://zod.dev/v4 |
| vite / @vitejs/plugin-react | 8.3.5 / 6.1.2 | 8.0.0: 2026-03-12 | yes | Rolldown-based | https://vite.dev/blog/announcing-vite8 |
| bun | 1.4.4 | 1.4.0: 2026-08-20 | yes | stable | npm `bun`; https://bun.sh/guides/ecosystem/vite |
| lucide-react | 1.62.0 | 1.0.0: 2026-03-23 | yes | stable | npm |
| typescript (context) | 7.0.2 | 7.0.2: 2026-07-08 | yes | Go-native TS 7 is now `latest`. Plugin compatibility is unverified (see below) | npm |

## Cited findings

### React and React Compiler
- React's `latest` tag is 19.3.1. React 19.3.0 shipped 2026-09-09 (npm).
- `babel-plugin-react-compiler@1.0.0` is `latest` (published 2025-10-07). The installation page says the compiler "is designed to work best with React 19, but it also supports React 17 and 18" (https://react.dev/learn/react-compiler/installation).
- With Vite 8 and `@vitejs/plugin-react` ≥6.0.0, "the inline Babel option was removed." The recommended setup is `@rolldown/plugin-babel` with `reactCompilerPreset()` from `@vitejs/plugin-react` (same URL). The react.dev pages I fetched don't state a stability level, so the "stable" label rests only on the npm `latest` tag pointing at 1.0.0.

### TanStack Router, Query, Form, Table, Virtual
- Router 1.x and Query 5.x have been on the same major since late 2023 (npm). Router says it is "Designed for client-side data caches (TanStack Query, SWR, etc.)" and lists "Path and Search Parameter Schema Validation" (https://tanstack.com/router/latest/docs/framework/react/overview).
- Form 1.x: "TanStack Form natively supports all libraries following the Standard Schema specification," including Zod. It supports async validators and `asyncDebounceMs` (https://tanstack.com/form/latest/docs/framework/react/guides/validation). Form depends on `@tanstack/react-store ^0.11.0` (npm `dependencies`). A 2.0 alpha is in progress (npm `alpha` tag 2.0.0-alpha.3).
- Table v9 is GA: 9.0.0 was published 2026-08-04, the same day as the last beta, 9.0.0-beta.80. The docs page is the v9 doc. It describes a "headless UI library ... to build your own fully customizable table and datagrid components," with modular, tree-shakable feature registration and v8 migration guides (https://tanstack.com/table/latest/docs/overview). The docs have no accessibility statement, so ARIA grid semantics are our responsibility.
- Virtual 3.x has been stable since 2023-12 (npm).

### TanStack DB and how it relates to Query
- DB "extends TanStack Query with collections, live queries and optimistic mutations." Collection types include Query, Electric, TrailBase, RxDB, PowerSync, IndexedDB, LocalStorage and LocalOnly (https://tanstack.com/db/latest/docs/overview).
- Query collections **wrap** Query: "Query Collections use TanStack Query internally." `queryClient` is required. "Query still handles fetching, caching, freshness, retries, and invalidation," while DB adds normalized rows, optimistic mutations and live queries. Parts of the API are "Transitional" or "deprecated and will be removed in v1.0." For robust real-time sync, the docs recommend a sync engine that owns ordering (https://tanstack.com/db/latest/docs/collections/query-collection).
- `@tanstack/offline-transactions` provides a durable outbox, FIFO retries with stable idempotency keys, and leader election via Web Locks or BroadcastChannel. The docs say local pending counts "do not prove the server applied a mutation" (https://tanstack.com/db/latest/docs/guides/offline-transactions).
- `@tanstack/tauri-db-sqlite-persistence` 0.2.37 and `@tanstack/browser-db-sqlite-persistence` 0.2.37 exist (npm). They fit the §78 Tauri mode, but they're 0.x. The offline-transactions page doesn't mention Tauri.

### TanStack Store, Pacer, Devtools
- Store: "primarily used for state management internally for most framework agnostic TanStack libraries. It can also be used as a standalone library" (https://tanstack.com/store/latest/docs/overview). 0.11.2 is current, with 1.0.0-alpha.0 published 2026-10-10.
- Pacer: "currently in beta and its API is still subject to change." It provides debounce, throttle, rate-limit, queue and batch (https://tanstack.com/pacer/latest/docs/overview).
- Devtools: "currently in alpha and its API is subject to change." `@tanstack/devtools-vite` provides "production build stripping" (https://tanstack.com/devtools/latest/docs/overview). Per-library panels exist on npm for Query, Router, Form, Pacer and Table. I found no DB devtools package (`@tanstack/react-db-devtools` and `@tanstack/db-devtools` aren't on npm).

### XState v5
- `xstate@5.33.3` is `latest`. Active v6 work is underway: `6.0.0-alpha.70` was published 2026-10-19 (npm; GitHub releases statelyai/xstate). `@xstate/react@6.1.0` declares peer `xstate ^5.28.0`.
- The docs document v5 (`createMachine`, `createActor`). `@xstate/react` provides the React hooks, and `@xstate/store` is a separate small store library (https://stately.ai/docs/xstate). The page doesn't say when v6 will be stable or how persistence works (unverified).

### Ark UI
- Homepage: "A headless library with 45+ accessible components … WAI-ARIA patterns baked in. Keyboard navigation and screen reader support out of the box … Driven by State Machines" (https://ark-ui.com/).
- `@ark-ui/react` depends on `@zag-js/*` 1.45.0 and `@internationalized/date` (npm `dependencies`). Zag says its machines and tests are "modelled according to the WAI-ARIA authoring practices" (https://zagjs.com/overview/introduction, via search snippet).
- Components (https://ark-ui.com/llms.txt): Accordion, Angle Slider, Avatar, Carousel, Checkbox, Clipboard, Collapsible, Color Picker, **Combobox**, **Date Input**, **Date Picker**, **Dialog**, Drawer, Editable, **Field**, **Fieldset**, File Upload, Floating Panel, Hover Card, Image Cropper, **Listbox**, Marquee, **Menu**, Navigation Menu, **Number Input**, Pagination, Password Input, Pin Input, Popover, Progress, QR Code, Radio Group, Rating Group, Scroll Area, Segment Group, **Select**, Signature Pad, Slider, Splitter, Steps, Switch, Tabs, **Tags Input**, Timer, Table of Contents, **Toast**, Toggle, Toggle Group, Tooltip, Tour, **Tree View**. **There is no Table or DataGrid.**
- I couldn't fetch per-component accessibility conformance tables (the docs URLs I tried returned 404). An independent WCAG 2.2 audit claim is **unverified**.

### Panda CSS
- 2.0 (2026-09-29) replaced the compiler with a Rust/Oxc engine. Extraction is reported as "15–37× faster." It adds bundler plugins `@pandacss/vite` and `@pandacss/bun`, source transforms that remove the styling runtime, publishable design systems (`panda lib` / `designSystem`), and an ESLint/oxlint plugin. It is "ESM-only and needs Node 22 or newer" (https://github.com/chakra-ui/panda/releases/tag/%40pandacss/dev%402.0.0).
- Vite setup: add `pandacss()` before `react()` in `vite.config.ts`, keep `"prepare": "panda codegen"` so `tsc` and the editor see `styled-system`, and declare `@layer reset, base, tokens, recipes, utilities;`. The guide includes Bun commands (https://panda-css.com/docs/installation/vite).
- Semantic tokens live in `theme.semanticTokens`, with conditional values such as `{ base, _dark }` emitted as CSS variables. A separate "Multiple Themes" page exists (https://panda-css.com/docs/theming/tokens). Slot recipes (`sva`) style multi-part components (https://panda-css.com/docs/concepts/slot-recipes). The page doesn't mention Ark UI anatomy, but Ark parts map onto slots naturally (inference). Density modes have no built-in feature. We'd build them ourselves as a condition or theme (inference).
- Park UI (`@park-ui/panda-preset` 0.43.1, last published 2024-11-22) is stale and predates Panda 2. Don't depend on it.

### Zod, Vite, Bun
- "Zod 4 is now stable!" It adds `z.toJSONSchema()` and Zod Mini (https://zod.dev/v4). The Standard Schema interop that TanStack Form and Router consume is confirmed from the Form side (Form validation guide above).
- Vite 8 "ships with Rolldown as its single, unified, Rust-based bundler" and "requires Node.js 20.19+, 22.12+" (https://vite.dev/blog/announcing-vite8).
- Bun: "Vite works with Bun with no extra configuration." `bunx vite` runs Vite under **Node** because of the shebang. `bunx --bun vite` runs it under Bun (https://bun.sh/guides/ecosystem/vite).

## Implications for the decision

### Depend on now (pin the major, allow minors)
React 19.3 with React Compiler 1.0, TanStack Router 1, Query 5, Form 1, Table 9, Virtual 3, XState 5 with `@xstate/react` 6, Ark UI 5, Panda CSS 2 (`@pandacss/vite`), Zod 4, Vite 8 with `@vitejs/plugin-react` 6, Bun 1.4, Lucide 1.

### Use with care or defer
- **TanStack DB:** don't make it a foundation yet. It's 0.x, and its docs mark transitional APIs for removal at 1.0. Its value is real for §101 (offline status, optimistic writes, sync/conflict) and §78 (Tauri SQLite persistence). Adopt it behind the `ClinicalRepository` adapter boundary, in a spike or one feature area first, so a breaking change stays inside the adapters. Don't adopt the Electric, TrailBase or PowerSync collections: the backend is Rust/FHIR, and those collections assume their own sync servers.
- **Offline outbox:** `@tanstack/offline-transactions` assumes idempotent server mutations and a single leader tab. Its semantics have to be reconciled with the FHIR conditional/transaction Bundle design and with the Edge Node's sync protocol. That belongs to the sync research tickets, not this one.
- **TanStack Pacer (beta):** fine for UI-local debounce and throttle (search-as-you-type for ValueSet, Practitioner and Medication pickers). Wrap it in one `useDebounced...` module so an API change costs one file. Form's own `asyncDebounceMs` covers validator debouncing, so don't use Pacer there.
- **TanStack Store (0.x, 1.0 alpha):** don't use it as an app-level store. Form already pulls it in transitively.
- **TanStack Devtools (alpha):** dev-only, wired through `@tanstack/devtools-vite` (production stripping). Breakage costs nothing in production, so it's acceptable. Verify in CI that the production bundle contains no devtools code.
- **Next majors** (Form 2, XState 6, Ark UI 6, Store 1.0): pin `^1`, `^5`, `^5`, `^0.11`. Don't adopt the alphas. Schedule an upgrade review when each one goes GA.

### State-ownership recommendation (satisfies §12, "avoid duplicating state")
| State kind | Owner | Notes |
|---|---|---|
| Server/FHIR resource cache | **TanStack Query** (keys per resource type, id and search params) | The `ClinicalRepository` adapter is the `queryFn`/`mutationFn`. |
| Normalized local collections, optimistic writes, offline outbox | **TanStack DB** *(later, if justified)* | Wraps the same QueryClient, so it doesn't duplicate Query. |
| URL state (patient id, tabs, filters, pagination) | **TanStack Router** search params, validated with Zod | |
| In-progress form data and drafts | **TanStack Form** | Autosave drafts go through the repository, not a second store. |
| Workflows and multi-step processes (encounter lifecycle, sign/finalize, sync status, conflict resolution, auth session) | **XState v5** actors | Machines hold *references* (ids) and invoke Query mutations. They must not copy resource data into their context. |
| Ephemeral widget state | Ark UI (Zag machines) or React `useState` | |
| Table view state (sorting, column visibility) | TanStack Table | Persist it via the Router URL or user prefs, not a separate store. |

Rule: each piece of data has exactly one owner. Other layers subscribe to the owner or hold its id.

### Design-system implications (Part 17 §99)
- Ark UI covers dialogs, menus, notifications (Toast), forms (Field, Fieldset, Combobox, Select, Date Picker, Number Input, Tags Input) and tree views. **Data tables** must be built from TanStack Table v9 and TanStack Virtual, with our own ARIA `grid`/`table` semantics and keyboard model. That component needs its own accessibility test effort.
- Use Panda 2 semantic tokens and conditions for light/dark. Implement clinical density modes as a custom condition (for example `[data-density=compact] &`) applied to spacing and size tokens. Use slot recipes per Ark component. Consider `panda lib` to publish the design system as a workspace package.
- Write our own recipes. Park UI is stale.

### Risks
1. **Panda 2 needs Node ≥22**, and **Vite runs under Node unless `--bun` is passed**. CI and dev machines therefore need Node 22+ even in a "Bun-only" setup, or must run everything with `bunx --bun`. Whether `@pandacss/vite` 2.x works when Vite runs on the Bun runtime is **unverified**. Run a scaffold spike.
2. **TanStack DB churn:** the 0.x core is released frequently (0.15.2 on 2026-10-21), and APIs are marked transitional.
3. **Accessibility evidence:** Ark UI's claims are self-asserted. Plan our own axe and screen-reader testing (NVDA/JAWS on Windows facility PCs, VoiceOver in the Tauri macOS build).
4. **Upcoming majors** in Form, XState, Ark UI and Store could land mid-build. Pinning plus upgrade windows mitigates this.
5. **TypeScript 7** (Go-native) is now `latest` (7.0.2). Whether Panda codegen, the TanStack Router plugin and React Compiler work with it is **unverified**. Default to TS 7 only if the scaffold spike passes. Otherwise pin TS 5.x.
6. **React Compiler with Vite 8** needs `@rolldown/plugin-babel`, which adds a Babel pass back into a Rust toolchain. Measure the build-time cost.

### Unverified
- Ark UI per-component WCAG conformance and any independent audit.
- XState v6 GA timeline and migration scope.
- TanStack DB 1.0 timeline. Tauri SQLite persistence behaviour on Windows facility hardware.
- `@pandacss/vite` 2.x running under the Bun runtime (`bunx --bun vite`). The `@pandacss/bun` plugin's fitness alongside Vite.
- Compatibility of the stack's codegen and plugins with TypeScript 7.
- React Compiler "stable" status beyond npm `latest` = 1.0.0 (the react.dev pages I fetched carry no explicit stability wording).
