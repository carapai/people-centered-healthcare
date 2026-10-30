# Adding a UI component

> Status: the Bun workspace (`apps/client`) and `@health/design-system` (`packages/design-system`, #45) exist. Feature modules and the DataTable are still to come.

## When to use

A reusable component in the design system, or a screen in a feature module of the one React app.

## Steps

1. **Reusable?** Put it in `@health/design-system` as a Panda CSS 2 slot recipe over an Ark UI 5 component: the recipe in `src/theme/recipes/`, the parts in `src/components/` (see `slot.tsx`), and an example in `example/gallery.tsx`. Park UI is not used. Tables use the `DataTable` (TanStack Table 9 + Virtual, ARIA grid, keyboard model). Icons come from Lucide.
2. **Tokens:** use semantic tokens (`bg.*`, `fg.*`, `border.*`, `accent.*`, `danger.*`; sizes `control` and `cell`), which carry the light, dark and high-contrast values and the compact density; no raw colours or sizes.
3. **Feature screen?** Put it in the Bounded Context's lazily loaded feature module, on the one route tree.
4. **Data:** read through `FhirClient` and write through the context's command port, via TanStack Query (`queryFn` / `mutationFn`). Never assemble FHIR writes for a business operation in the UI.
5. **State ownership:** URL state in TanStack Router (Zod-validated search params), field drafts in TanStack Form, processes in XState v5 actors that hold **ids only**, widget state in Ark UI or `useState`.
6. **Writes wait for confirmation** (clinical and financial); only non-clinical preferences may be optimistic. On Tauri, show the sync item badge.
7. **No runtime checks:** read `Platform.capabilities`; never import `@tauri-apps/*` outside `src/platform/`.
8. **No clinical data in storage:** nothing clinical in localStorage, IndexedDB, OPFS or a service worker (ADR 0037).
9. **Accessibility:** WCAG 2.2 AA; touch targets for tablets; works at 1366×768.
10. **Text:** ICU messages through react-intl: ids `ds.<component>.<name>` in the design system, `app.<feature>.<name>` in the client, each with a description. No literal text in components.

## Expected files

- `packages/design-system/`: `src/theme/recipes/<name>.ts`, `src/components/<name>.tsx`, the example in `example/gallery.tsx`, and tests in `test/`
- `apps/client/src/…` feature module files (directory convention to be fixed with the first feature)
- Message entries in `src/i18n/messages.ts` of the package

## Contracts

`Platform` (transport, session, events, capabilities); `FhirClient`; per-context command ports with generated TS types and Zod schemas; Query keys per resource type, id and search.

## Tests

- **Frontend:** Vitest unit tests, component tests, axe accessibility checks (every PR); manual NVDA testing on Windows.
- `frontend-ci.yml`: `tsc`, lint (including the `src/platform` import rule), `panda codegen`, production build, bundle scan (no `@tauri-apps`, no devtools); for the design system, component and axe tests in Chromium over the example page in every theme and density.
- **End-to-end:** Playwright on the web, `tauri-driver` on WebView2 and WebKitGTK (nightly).

## Governing decisions

[ADR 0036](../adr/0036-ports-over-swappable-transport-with-tauri-ipc-into-the-shared-router.md), [ADR 0037](../adr/0037-no-clinical-data-in-browser-or-webview-storage.md), [ADR 0038](../adr/0038-fhirpath-js-for-live-form-behaviour.md), [client-application.md](../architecture/client-application.md), [research/frontend.md](../research/frontend.md).
