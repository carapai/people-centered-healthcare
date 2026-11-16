# Research: graph-canvas library for the Workflow Studio

Ticket: [#480 Which graph-canvas library fits the Workflow Studio?](https://github.com/carapai/people-centered-healthcare/issues/480). Facts checked on 2026-11-16 against the npm registry, GitHub and the projects' own docs.

**Recommendation:** use **`@xyflow/react` (React Flow) 13.x** for the canvas and **`@dagrejs/dagre` 3.x** for auto-layout. Do not adopt `elkjs` without an ADR, because its licence is not on ADR 0045's allow-list.

## Requirements

- MASTER_PROMPT §52 asks for a visual editor with nodes, connectors, guards, forms, timers, role assignments, actions, branches, reusable subprocesses, simulation and publishing.
- The statechart model in `docs/architecture/workflow-engine.md` (ADRs 0042–0044) has `atomic`, `compound`, `parallel` and `final` states. The canvas therefore needs nested group nodes, edges that cross group boundaries, and regions inside a parallel state.
- ADR 0045 allows these licences: MIT, Apache-2.0, BSD, ISC, Unicode, Zlib and MPL-2.0. BSL 1.1 is allowed only for SurrealDB. Any copyleft licence needs an ADR. `deny.toml` also allows MIT-0 and 0BSD. On the JS side, `.github/workflows/dependency-review.yml` enforces the list.
- `apps/client/package.json` pins React 19.3.1 and Panda CSS 2.4.0.
- MASTER_PROMPT line 507 forbids Tailwind CSS, Shadcn UI, Ant Design, Chakra UI and Material UI.
- The UI must meet WCAG 2.2 AA.

## Comparison

Bundle sizes are min+gzip. I measured them with esbuild 0.x in a scratch directory outside the repo, with `react` and `react-dom` external. They are indicative, not tree-shaken app numbers.

| | **@xyflow/react 13.0.1** | @joint/core 4.3.4 (+ @joint/react 4.3.7) | Rete.js 2.0.6 (+ rete-react-plugin 2.1.2) | @maxgraph/core 0.25.1 | Hand-rolled SVG |
|---|---|---|---|---|---|
| Licence | MIT. Transitive deps are MIT, ISC and BSD-3-Clause: **all allowed** (see below) | MPL-2.0: allowed | MIT: allowed | Apache-2.0: allowed | n/a |
| React 19 | Native React components. Peers `react >=18`. v13 ships compiled with React Compiler (`react-compiler-runtime`) | `@joint/react` peers `react >=18`. The core is an imperative SVG/Backbone-style model wrapped for React | Peers `^19` are allowed, but the plugin **peers `styled-components`** | No React binding. You manage an imperative DOM island yourself | Yes |
| Keyboard / screen reader | Built in: nodes and edges are focusable, Tab moves between them, Enter/Space selects, Escape deselects, arrows move nodes (Shift moves faster), Delete removes, focus auto-pans. It also has `ariaRole`, `domAttributes`, `aria-label` on nodes and edges, an `aria-live` region and `ariaLabelConfig` for i18n. The docs claim WCAG 2.1 AA support only "when properly implemented" | Nothing documented in the open-source core. The Keyboard plugin is in the commercial JointJS+ | Nothing documented | Nothing documented. It inherits mxGraph's mouse-first model | Everything is yours to build |
| Panda CSS, no Tailwind | Ships plain CSS (`base.css`/`style.css`) with CSS variables. Custom nodes and edges are your own React components, so you style them with Panda recipes. No Tailwind | Styling is through SVG attributes and markup in JS. Panda only applies inside HTML/`foreignObject` | `styled-components` is a second, runtime CSS-in-JS system next to Panda: **poor fit** | Style objects in JS. Panda does not apply | Full control |
| Compound / parallel states | Sub-flows: `parentId`, `extent: 'parent'`, positions relative to the parent, any depth. v13 drops the rule that parents come before children in the array. Parallel regions can be child group nodes | `embed`/`unembed` parent and child cells, and good nesting | No native groups. Needs a scopes plugin | Native nested cells and swimlanes | Yours to build |
| Auto-layout | None built in. Official examples use dagre (sub-flows supported, with a known caveat for edges leaving a sub-flow) and elkjs (sub-flows fully supported). d3-hierarchy and d3-force do not support sub-flows | `@joint/layout-directed-graph` (dagre) and `@joint/layout-elk` | Auto-arrange plugin (elkjs) | Built-in hierarchical and tree layouts | dagre or elkjs |
| Bundle (min+gz) | about 86 KB for the typical import set, about 91 KB for everything | about 143 KB, plus the React wrapper | about 13 KB for core and area, plus the React plugin and styled-components | about 109 KB for `Graph` | Smallest, but highest build cost |
| Maintenance | 39k stars, 126 open issues. Releases every 2–6 weeks through 2026 (12.9 → 12.12.1). **13.0.0/13.0.1 released 2026-11-16.** Backed by webkid GmbH, funded by React Flow Pro | 5.4k stars, 67 open issues. Monthly releases. Backed by client IO, which sells JointJS+ | 12.3k stars. **Core last released 2025-06-30.** React plugin 2026-07 | 1.2k stars. Still pre-1.0 (0.25.1, 2026-10-21). About quarterly releases | You maintain it |

Layout-only engines:

| | @dagrejs/dagre 3.1.3 | elkjs 0.12.1 |
|---|---|---|
| Licence | MIT (graphlib 4.0.x MIT): allowed | **EPL-2.0 OR GPL-3.0-or-later**: **not on the allow-list**. Both options are copyleft (EPL is weak copyleft), so it needs an ADR |
| Compound graphs | Yes (graphlib `compound: true`, `setParent`). React Flow notes a caveat for edges that cross out of a sub-flow | Yes, the strongest hierarchical and compound layout |
| Size (min+gz) | about 17 KB | about 450 KB (the bundled GWT build). Run it in a worker |
| Maintenance | v3 line actively released (3.1.3, 2026-10-24) | Releases in 2026-03, 2026-07 and 2026-10 |

## Transitive licences of `@xyflow/react@13.0.1`

I walked the full dependency tree from `node_modules` after a scratch install. All of these are allowed:

- `@xyflow/system` 1.0.0 MIT
- `zustand` 5.1.0 MIT
- `classcat` 5.0.5 MIT
- `react-compiler-runtime` 1.0.0 MIT
- `d3-drag`, `d3-zoom`, `d3-selection`, `d3-interpolate`, `d3-color`, `d3-dispatch`, `d3-transition` and `d3-timer` (3.x): ISC
- `d3-ease` 3.0.2: BSD-3-Clause
- `@types/d3-*`: MIT

The repository's own LICENSE file is MIT (© webkid GmbH). React Flow Pro is a paid subscription for **examples and support only**. The library itself has no paywalled features. The pro page says "React Flow is open-source MIT-licensed software, and it will be forever". The canvas shows a small "React Flow" attribution link. The MIT licence does not require it, but the maintainers ask that only Pro subscribers remove it ([remove-attribution](https://reactflow.dev/learn/troubleshooting/remove-attribution)). Keeping the attribution, or subscribing, is a product decision. It is not a licence issue.

## Why React Flow

1. It is the only candidate that is natively React and also covers both requirements that are expensive to build: **keyboard and screen-reader support** and **nested groups**. It fits the Panda stack because every node and edge is a React component we write.
2. Its licence and dependencies are clean against ADR 0045, with no new licence family.
3. It is the healthiest project: the largest user base, the fastest release cadence, and commercial backing that does not gate features.
4. The rejected options each fail on a specific point:
   - JointJS: its keyboard module is commercial, and it is SVG-model-first.
   - Rete: styled-components, and a stalled core.
   - maxGraph: pre-1.0, no React binding, no a11y.
   - Hand-rolled: months of work on pan, zoom, edges, selection and a11y.

## Risks and how to close them

- **v13 is one day old.** Option 1 is to start on 13.0.x and pin exactly, since the first patch release already landed the same day. Option 2 is to pin 12.12.1 and plan a migration later. Starting on 13 avoids a breaking migration: the store split, `NodeChangeset`, and the removed `useStore`, `nodeClickDistance` and `getHandleConnections`. I would start on 13 and pin the exact version, following the "pin the current majors" rule in `docs/research/frontend.md`.
- **The a11y support is a floor, not WCAG 2.2 AA.** To close the gap:
  - Give every custom node an `aria-label` (state name, kind, and for compound or parallel states the child count) and `aria-roledescription` through `domAttributes`.
  - Localise `ariaLabelConfig`.
  - Provide a **keyboard way to create connections**. Dragging handles is pointer-only, so this is needed for WCAG 2.5.7 Dragging Movements. For example, a "connect to…" combobox (Ark UI) on the focused node.
  - Provide a synchronised **non-visual outline**: an Ark UI tree view of states, transitions, guards and timers, as an equivalent alternative for screen-reader users. A 2-D canvas is hard to understand by Tab order alone.
  - Check focus visibility and contrast (2.4.11 Focus Not Obscured, 1.4.11 Non-text Contrast) against Panda tokens.
  - Add axe and keyboard tests in Playwright.
- **Auto-layout of compound states.** dagre is allowed and small, but its compound layout misplaces edges that leave a sub-flow. Options: lay out each compound state's children bottom-up (inner first, then the parent as a fixed-size box), or write an ADR that accepts elkjs under EPL-2.0 and load it lazily in a Web Worker for the Studio route only. I recommend starting with nested dagre and revisiting only if the layouts are poor.
- **Default CSS.** Import only `@xyflow/react/dist/base.css` (structural) and theme nodes and edges with Panda tokens. v13 follows the system colour mode by default. Drive it from our theme with `colorMode` or `forceColorMode`.
- **Bundle.** About 86 KB gz. Lazy-load the Studio route with TanStack Router code-splitting so that clinical screens never pay for it.

## Sources

- npm registry metadata via `npm view <pkg> version license dependencies peerDependencies time`, for every package and version above (2026-11-16).
- xyflow LICENSE (MIT): https://github.com/xyflow/xyflow/blob/main/LICENSE
- React Flow 13.0.0 release notes: https://github.com/xyflow/xyflow/releases/tag/%40xyflow/react%4013.0.0
- React Flow accessibility: https://reactflow.dev/learn/advanced-use/accessibility
- React Flow sub-flows: https://reactflow.dev/learn/layouting/sub-flows
- React Flow layouting (dagre, elkjs, d3-hierarchy, d3-force; sub-flow support table): https://reactflow.dev/learn/layouting/layouting
- React Flow Pro: https://reactflow.dev/pro
- JointJS core (MPL-2.0): https://github.com/clientIO/joint
- JointJS+ Keyboard plugin (commercial): https://docs.jointjs.com/api/ui/Keyboard
- JointJS on diagram accessibility: https://www.jointjs.com/blog/diagram-accessibility
- Rete.js: https://github.com/retejs/rete and https://retejs.org/
- maxGraph: https://github.com/maxGraph/maxGraph
- elkjs (EPL-2.0 OR GPL-3.0-or-later): https://github.com/kieler/elkjs
- dagre: https://github.com/dagrejs/dagre
- WCAG 2.2, 2.5.7 Dragging Movements: https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html
- Repo: `docs/adr/0045-pinned-least-privilege-ci-and-signed-or-failed-releases.md`, `deny.toml`, `.github/workflows/dependency-review.yml`, `apps/client/package.json`, `docs/architecture/workflow-engine.md`, `MASTER_PROMPT.md` §52 and line 507.

## Not verified

- I did not check screen-reader behaviour hands-on (NVDA or VoiceOver). The a11y claims come from React Flow's docs and from the presence of the props in the 13.0.1 source.
- I did not build a prototype to confirm that React Flow renders parallel regions inside a compound state well. The capability comes from the documented sub-flows API.
- Bundle sizes are esbuild measurements of the entry points, not of the final Vite/Rolldown app chunk.
