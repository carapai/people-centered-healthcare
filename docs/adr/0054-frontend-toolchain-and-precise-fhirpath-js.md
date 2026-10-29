# The frontend runs on Bun with TypeScript 7, and fhirpath.js runs in lossless precise mode over a gated expression subset

This records the results of the spike [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38), run 2026-10-29 on macOS x86_64.

**Toolchain:**
- **Bun only.** Dev and build run under `bunx --bun vite`, and Node is not required. Vite 8.3 with Rolldown builds the minimal app in about 0.5 s.
- **TypeScript 7.** We pin `typescript@7.0.2` (the Go-native compiler, published as the plain `typescript` package).
  - The app, the generated Panda `styled-system` and `routeTree.gen.ts` type-check with full library checking: 0 errors in 1.0 s. TS 5.9.3 took about 2 minutes.
  - Tooling config files (`vite.config.ts`, `panda.config.ts`) are type-checked in a separate `tsconfig.node.json` with `skipLibCheck`, because third-party Babel 7/8 and Panda type declarations clash. TS 5.9.3 reports the same 20 errors.
- **Panda CSS 2.4** works through `@pandacss/vite`. The config must list `@pandacss/preset-base` and `@pandacss/preset-panda` explicitly, and CI checks that emitted CSS contains token variables.
  - Without the presets, Panda emits invalid CSS silently, and `panda doctor` still reports ok.
- **React Compiler 1.0** runs through `@rolldown/plugin-babel` and does transform components. It costs about 15 ms of build time per component file, which we accept.
- **Tauri.** A Tauri 2.12 command dispatching into the shared Axum 0.8 router with `tower::ServiceExt::oneshot` returns the same status, JSON body and content type as real HTTP (ADR 0036). Commands are declared in a module, not the crate root, because the macro collides there (E0255).

**fhirpath.js 5.2:**
- **Lossless precise mode.** It runs only with resources parsed by `lossless-json`, and with `preciseMath` and `keepDecimalTypes` enabled.
  - With plain `JSON.parse`, 1.50 becomes 1.5, a 30-digit decimal becomes `1.2345678901234568e+29`, and two different 30-digit values compare equal.
  - Lossless precise mode is the only configuration that keeps the source text, as ADR 0052 requires on the server.
- **Gated expression subset.** fhirpath.js passed 1,139 of the official R5 suite's 1,307 tests. Not implemented: `precision()`, `conformsTo()` and the document functions. It also:
  - returns true or false instead of empty for mixed-precision and unit-mismatch comparisons;
  - mishandles boundary functions;
  - drops the original timezone offset in date arithmetic.

  Expressions in published Questionnaires may therefore use only constructs whose shared conformance fixtures pass in both engines. The publication gate of ADR 0038 enforces this, and the failing groups are listed in [docs/research/frontend.md](../research/frontend.md). Browser results remain previews, and the server recomputes calculated values (ADR 0052).

We chose this because every pinned piece worked under Bun, and TS 7 is fifty to a hundred times faster than TS 5.9 on the same code. The fhirpath.js gaps are real but confined to functions that live forms do not need, and the gate catches any form that relies on them.

## Considered Options

- **Node 22 for dev and CI:** not needed. Rejected.
- **Pinning TypeScript 5.9:** the same library-type errors, and much slower. Rejected.
- **Running FHIRPath on the server for every keystroke:** already rejected in ADR 0038.

## Consequences

- The scaffold task [Create the Bun workspace and the React app shell with the platform boundary](https://github.com/carapai/people-centered-healthcare/issues/44) carries these pins and checks.
- `lossless-json` is a client dependency, and FHIR resources the renderer evaluates are parsed through it.
- Each fhirpath.js bump reruns the official suite. A newly passing group can widen the allowed subset; a newly failing one blocks the bump.
