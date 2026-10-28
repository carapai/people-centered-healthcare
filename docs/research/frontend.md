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

- Node 22 may be needed for Panda 2 even in a Bun setup, and TS 7 compatibility is unknown. Both are checked by [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38).
- Upcoming majors may land mid-build, so pin and review upgrades when each goes GA.
- Accessibility evidence needs our own axe and NVDA testing.
