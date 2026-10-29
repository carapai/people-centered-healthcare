# Research: frontend stack stability

A condensed version of the research ticket [Which TanStack, XState, Ark UI and Panda CSS pieces are stable enough to depend on?](https://github.com/carapai/people-centered-healthcare/issues/12). The full findings, with citations and npm dates, are on the unmerged branch `research/frontend`. Decisions: [client-application.md](../architecture/client-application.md) (ADRs 0036–0038).

## Versions as of 2026-10-27

| Stable (≥1.0), so depend on it | Pre-1.0, so only "where justified" |
|---|---|
| React 19.3 and React Compiler 1.0 | TanStack DB 0.15 / react-db 0.5 (transitional APIs to be removed at 1.0) |
| TanStack Router 1.170, Query 5.105, Form 1.33 | TanStack Store 0.11 (1.0 alpha) |
| TanStack Table 9.3 (GA 2026-08-04), Virtual 3.14 | TanStack Pacer 0.23/0.24 (beta) |
| XState 5.33, `@xstate/react` 6.1 | TanStack Devtools 0.10 (alpha) |
| Ark UI 5.39 (on Zag 1.45) | `@tanstack/tauri-db-sqlite-persistence` 0.2 |
| Panda CSS 2.4 (Rust/Oxc engine; ESM-only; Node ≥22) | |
| Zod 4.6, Vite 8.3 (Rolldown), Bun 1.4, Lucide 1.62 | |

Prerelease next majors exist for Form 2, XState 6, Ark UI 6 and Store 1.0. Pin the current majors.

## Key facts

- **Ark UI** has 45+ WAI-ARIA components driven by state machines, including combobox, date picker, dialog, menu, toast and tree view. It has **no table or data grid**. Its accessibility claims are self-asserted.
- **TanStack Table 9** is headless. ARIA grid semantics are the implementer's job.
- **TanStack DB** wraps Query rather than replacing it. `offline-transactions` provides a durable outbox with leader election, and its docs say that local pending counts "do not prove the server applied a mutation".
- **Panda 2:**
  - `@pandacss/vite`, `panda codegen`, `panda lib` for design-system packages;
  - semantic tokens with conditions;
  - slot recipes for multi-part components;
  - no built-in density modes;
  - Park UI is stale (last published 2024-11).
- **Vite 8 and React Compiler:** the inline Babel option was removed in `@vitejs/plugin-react` 6. Use `@rolldown/plugin-babel` with `reactCompilerPreset()`.
- **Bun:** `bunx vite` runs Vite under Node; `bunx --bun vite` runs it under Bun.
- **TypeScript 7** (Go-native) is `latest`. Plugin compatibility is unverified.

## Risks carried into the design

- Node 22 and TS 7 risks are resolved: neither is a problem (spike below).
- Upcoming majors may land mid-build, so pin and review upgrades when each goes GA.
- Accessibility evidence needs our own axe and NVDA testing.

## Scaffold spike (2026-10-29)

The spike was [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38), on macOS x86_64 with Bun 1.4.2 and rustc 1.98.0. The decision is ADR 0054.

**Resolved versions:**
- **Build:** vite 8.3.5 (rolldown 1.2.15), `@vitejs/plugin-react` 6.1.2, `@rolldown/plugin-babel` 0.2.4, `babel-plugin-react-compiler` 1.0.0, `@babel/core` 8.0.9.
- **App libraries:** React 19.3.1, `@tanstack/react-router` 1.170.46 with `@tanstack/router-plugin` 1.168.48, Panda 2.4.0.
- **Other JS:** `typescript` 7.0.2, `fhirpath` 5.2.2, `lossless-json` 4.3.3.
- **Rust:** tauri 2.12.5, axum 0.8.9, tower 0.5.3.

| Check | Result |
|---|---|
| `bunx --bun vite` dev and build | Pass. The build takes about 0.5 s and the dev server is ready in 0.7 s. Node 22 also works but is not needed |
| Panda codegen and Vite plugin | Pass with explicit presets. Codegen takes 0.31 s for 131 files. Without presets the CSS is silently invalid and `panda doctor` still reports ok |
| `tsc` 7.0.2 | App, `styled-system` and `routeTree.gen.ts` give 0 errors in 1.0 s. Config files give 20 errors in third-party types (Babel 7/8 clash, Panda re-exports), the same as TS 5.9.3, which took 112–123 s |
| React Compiler | Applied (`react/compiler-runtime`, `_c(n)`). 300 components build in 4.86 s with it and 0.30 s without |
| Tauri `oneshot` into Axum versus real HTTP | Pass on GET Patient (200), POST command (201) and a 404 OperationOutcome. Cold compile 61 s, incremental 4.6 s |
| fhirpath.js on the official R5 suite (fhir-test-cases c56d442) | 1,139 pass, 162 fail, 6 skipped (CDA and terminology) |

**fhirpath.js failing groups:**
- **Not implemented:** document functions (65), `Precision` (6), `conformsTo` (2).
- **Static semantic checks it does not do:** 25.
- **Wrong or missing behaviour:**
  - boundary functions (15);
  - mixed-precision and unit-mismatch comparisons return true or false instead of empty (about 10);
  - `testPlus` keeps the instant but loses the source offset (11);
  - `distinct` (7);
  - `%sct`, `%loinc`, `%vs-*` and `%ext-*` are undefined (4);
  - a few other functions.

**fhirpath.js decimals:**
- With plain `JSON.parse`, 1.50 becomes 1.5 and 0.010 becomes 0.01, a 30-digit value becomes `1.2345678901234568e+29`, and two different 30-digit values compare equal.
- With `lossless-json`, `preciseMath` and `keepDecimalTypes`, the source text is kept exactly.

**Not run:**
- a GUI or webview, and the JS-side `invoke`;
- HMR;
- `panda lib` and `panda build`;
- terminology and CDA tests.

