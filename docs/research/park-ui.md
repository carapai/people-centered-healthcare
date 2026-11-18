# Research: Park UI with Panda CSS 2 and Ark UI 5

Findings for the research ticket [Is Park UI current and compatible with Panda CSS 2 and Ark UI 5, and what does its theming offer?](https://github.com/carapai/people-centered-healthcare/issues/644). This note reports facts and decides nothing. The map is #643. Related design: [client-application.md § Design system](../architecture/client-application.md#design-system), [frontend.md](frontend.md) (which said in October 2026 that "Park UI is stale (last published 2024-11)").

Research date: **2026-11-18**. Every source was read on that date. Sources are the npm registry, the Park UI, Panda and Chakra GitHub repositories, and their official websites. Claims marked **unverified** have no primary source yet. Our versions: `@pandacss/dev` 2.4.0 and `@ark-ui/react` 5.39.3 in `packages/design-system` and `apps/client`.

## 1. Summary

| Topic | Key facts | Section |
|---|---|---|
| State | `@park-ui/panda-preset` is dead: last version 0.43.1 on 2024-11-22, and the 2025-11-21 changelog says "goodbye". Park UI now ships as a **CLI that copies recipes and components into your project** (`@park-ui/cli` 1.0.1, 2025-11-20). The repo moved to `chakra-ui/park-ui` (announced 2025-11-27). Last commit on `main`: 2026-02-21. No release since 2025-11-20. Licence MIT. | §2 |
| Panda 2 | Park UI's source targets **Panda 1.8.1** and **Ark UI 5.30.0**. Open issue #577 (2026-11-06) asks for a Panda 2 migration, with no reply. Under Panda 2.4 the recipes and theme build (`codegen`, `cssgen`, `panda lib`) with one warning. The React components do not type-check: all 41 use `createStyleContext`, which Panda 2 split into `createSlotRecipeContext` and `createRecipeContext`. A rename fixed it in a small test. | §3 |
| Theming | Radix Colors: 31 palettes, 12 steps per mode plus 12 alpha steps, light and dark values in one token. One accent and one gray chosen at `init`. Shared variants `solid`, `subtle`, `surface`, `outline`, `plain`. Radius by three semantic tokens `l1`–`l3`. No font choice. All values are build-time Panda tokens emitted as CSS variables. | §4 |
| Runtime switching | Recipes use Panda's virtual `colorPalette`. A palette class such as `.color-palette_teal` only re-points `--colors-color-palette-*` variables, so an app could switch accent at runtime if the palette CSS is generated in advance. Overriding `--colors-*` variables also works without a rebuild. Park UI documents neither. | §4.3 |
| Dark, contrast, density | Dark mode by the `.dark` class (Panda 2 preset-base). No high-contrast theme. No density mode: sizes are recipe variants (`2xs`–`2xl`; `md` input is 40 px). | §5 |
| Accessibility | Park UI claims "accessible" through Ark UI; it publishes no contrast figures. Radix says steps 11 and 12 reach APCA Lc 60 and Lc 90 on step 2 (APCA, not WCAG). | §6 |
| Components | 62 React components including Field, Select, Combobox, Dialog, Menu, Toast, Date Picker, Table, Tabs, Card, Badge, Avatar, Skeleton. No DataTable (a `table` recipe only), no sidebar or app shell, no empty state. | §7 |
| Alternatives | `@chakra-ui/panda-preset` 3.38.0 (2026-11-03, MIT) is Chakra UI 3's theme as a Panda preset, 76 recipes including empty state, data list and tree view. It builds under Panda 2.4 with seven warnings. It has recipes only; Chakra's React components need its Emotion runtime. Copying Park UI recipes as source is the vendor's own model since v1. | §8 |

## 2. Current state

### 2.1 Packages on npm

| Package | Latest | Published | Notes |
|---|---|---|---|
| `@park-ui/panda-preset` | 0.43.1 | 2024-11-22 | peer `@pandacss/dev >0.22.0`; depends on `@ark-ui/anatomy` 3.5.0. Retired (§2.2). |
| `@park-ui/cli` | 1.0.1 | 2025-11-20 | `1.0.0` the same day. Prereleases `1.0.0-next.0` to `next.8` from 2025-08-10 to 2025-11-17. Peer `typescript ^5`. |
| `@park-ui/tailwind-plugin` | 0.20.1 | 2024-03-06 | Old Tailwind variant. |
| `@park-ui/react`, `@park-ui/preset` | not published | | Private workspace packages in the repo. |

Third-party forks of the old preset exist (`@dcousineau/park-ui-panda-preset` 0.43.3, 2025-04-14; `@mgoodfellow/park-ui-panda-preset` 0.43.2-fix-accent-colors.0, 2025-06-13). They are not official.

### 2.2 Distribution model

The changelog of 2025-11-21 ("Park UI Gets a Major Upgrade") says:

- "We're saying goodbye to `@park-ui/panda-preset`… Now when you add a component, you'll get its recipe too, right in your codebase."
- `RadioButtonGroup` became `RadioCardGroup`; `FormLabel` was folded into `Field`.
- Next on the roadmap: `DatePicker`, `ColorPicker`, `TreeView`. All three now exist in the repo (§7).

The installation page: install `@ark-ui/react` and `lucide-react`, run `npx @park-ui/cli init`, then `npx @park-ui/cli add button`. `init` asks for framework (React or Solid), accent colour, gray colour and border radius. It fetches registry items (`__init`, the accent, the gray) from `park-ui.com/registry/<framework>/*.json` and merges config into `panda.config.ts`. `__init` writes theme files (conditions, global CSS, keyframes, layer and text styles, tokens). Each component item brings its recipe file. The registry has 307 files for React and Solid. Vue has components in the repo but is not in the CLI prompt.

In the CLI source the border-radius answer is not used: `prompt.ts` always writes `l1 = radii.xs`, `l2 = radii.sm`, `l3 = radii.md` (read at commit `102769c`).

### 2.3 Repository activity

- `github.com/cschroeter/park-ui` now redirects to `github.com/chakra-ui/park-ui`. The post "Park UI Joins the Chakra UI Organization" (2025-11-27) says the creator is a Chakra core member and "Nothing breaks".
- 2,385 stars, 25 open issues, not archived, licence MIT.
- Last commit on `main`: 2026-02-21 ("fix(react): use specific paths imports from @ark-ui/react (#554)"). Commits from December 2025 to February 2026 are fixes and a new `DisplayValue` component.
- `pushed_at` 2026-04-10 is a Dependabot branch. Open PRs from April to July 2026 (#566, #567, #568, #572, #574, #575) have no merge.
- Open issue **#577 "Plans for Panda v2 migration?"** (2026-11-06), zero comments. Also open: Solid core v2 (#576), Vue/Nuxt (#551), custom `outDir` (#548).
- The `migration.mdx` docs page says only "Learn how to migrate from Park UI to Park UI" (description "TBD").

## 3. Compatibility with Panda CSS 2 and Ark UI 5

### 3.1 What Park UI's source pins

At `main` (`102769c`, 2026-02-21):

- `components/react/package.json`: `@ark-ui/react` 5.30.0, `@pandacss/dev` 1.8.1, `@pandacss/preset-base` 1.8.1, `@pandacss/preset-panda` 1.8.1, `lucide-react` 0.563.0, React 19.2, TypeScript 5.9.3, Vite 7.3.1.
- `packages/preset/package.json`: `@pandacss/dev` 1.8.1, `@radix-ui/colors` 3.0.0.
- Components import `createStyleContext` from `styled-system/jsx`.
- The preset uses `presets: ['@pandacss/preset-base', '@pandacss/preset-panda']` and a plugin with a `preset:resolved` hook that drops Panda's 50–950 colours.

Current versions: `@pandacss/dev` 2.5.0 (2026-11-02; 2.0.0 was 2026-09-29, 2.4.0 2026-10-22; v1 line 1.12.2), `@ark-ui/react` 5.40.0 (`next` 6.0.0-next.0).

### 3.2 Panda 1 → 2 breaking changes that touch Park UI

From the Panda 2.0 announcement (2026-09-29, "Upgrading from v1"):

- **ESM only, Node 22+.** Park UI's config files are ESM already.
- **`createStyleContext` split** into `createSlotRecipeContext` (slot recipes) and `createRecipeContext` (single recipes); a new `withRootProvider` exists. Every Park UI component that uses it breaks.
- **Hooks moved into plugins** with `filter` and `handler`; engine-internal hooks are gone. Park UI's `preset:resolved` plugin hook still worked in the test (§3.3).
- **Removed options**: `studio`, `eject`, `emitTokensOnly`, `gitignore`, `clean`, `watch`, `poll`, `lightningcss`, `browserslist`, `syntax`. Park UI's configs do not use them in the React package. The React `prepare` script runs `panda codegen --clean --silent`; in v2 `--silent` became `--log-level silent` (**unverified** whether `--clean` survives on `codegen`).
- **`panda ship` removed**; `panda lib` plus `designSystem` replace it.
- CSS output changes: media-range syntax, deterministic layer order, `@property` variables, narrower `sizes.prose`.

### 3.3 Build test (scratch directory, not the repo)

Setup: Bun 1.4.2, Node 22.16. Installed `@pandacss/dev`, `preset-base`, `preset-panda` 2.4.0, `@ark-ui/react` 5.39.3, React 19, TypeScript 5. Copied `packages/preset/src` (theme, 31 colours, 67 recipes) and 17 React components from `components/react/src/components/ui` at `102769c`.

| Step | Result |
|---|---|
| `panda codegen` | Wrote 263 files. One warning: `theme.slotRecipes.checkbox: control in control.borderColor is not a condition or selector… the styles under it never apply`. |
| `panda cssgen` with `staticCss.recipes: '*'` | 331 KB CSS. Panda's own `--colors-red-500` absent, so the colour-removal plugin worked. Radix `--colors-red-9` present. |
| `panda lib --include './src/**/*.{ts,tsx}'` | Wrote `dist/panda/lib.json` and `preset.mjs`, updated `exports`. Same warning. |
| `tsc` on the components | `TS2305: Module "styled-system/jsx" has no exported member 'createStyleContext'` in combobox, date-picker, dialog, field, menu, select, table, toast. |
| After `sed s/createStyleContext/createSlotRecipeContext/` | No type errors left except a missing copied file. Not run in a browser; behaviour is **unverified**. |

The test did not use `@park-ui/cli` itself (its peer is `typescript ^5`; we use TypeScript 7). Whether the CLI runs against a Panda 2 config is **unverified**.

## 4. Theming model

### 4.1 Colours

- Built on **Radix Colors**. The theming page: "Instead of the 50–950 scale used in Panda CSS, Park UI is built on Radix Colors. Each color comes with 12 shades per mode (light and dark)."
- 31 palettes in `packages/preset/src/theme/colors/`: amber, blue, bronze, brown, crimson, cyan, gold, grass, green, indigo, iris, jade, lime, mauve, mint, neutral, olive, orange, pink, plum, purple, red, ruby, sage, sand, sky, slate, teal, tomato, violet, yellow.
- Grays offered by the CLI: neutral, mauve, slate, sage, olive, sand. The chosen gray is installed as `colors.gray`.
- Each palette file is a semantic token set: steps `1`–`12` and `a1`–`a12` (alpha), each `{ _light, _dark }`, plus variant aliases `solid.bg`, `solid.bg.hover`, `solid.fg`, `subtle.*`, `surface.*`, `outline.*`, `plain.*`.
- Global aliases: `fg.default` (gray 12), `fg.muted` (11), `fg.subtle` (10), `canvas` (gray 1), `border` (gray 4), `error` (red 9).
- "One accent color and one gray color" ("Pairings"). `html` gets `colorPalette: 'gray'` in global CSS; components take a `colorPalette` prop or style.

### 4.2 Radius, fonts, sizes, variants

- Radius: semantic tokens `l1`, `l2`, `l3` (nested corners) mapped to Panda's `radii.*`.
- Fonts: none. The Panda preset's `sans`, `serif`, `mono` stacks apply. Text styles `xs`–`7xl` and `label`.
- Sizes: "primary" components (Button, Input, Select) share heights per size; `md` Button and Input are 40 px. Input sizes `2xs`–`2xl` map to `sizes.7`–`sizes.16`.
- Variants: `solid`, `subtle`, `surface`, `outline`, `plain`, shared across components.
- Shadows `xs`–`2xl` with light and dark values. z-index tokens included.

### 4.3 Build time or runtime

- All theme values are Panda tokens, fixed when Panda builds. The CLI writes them as source.
- Panda emits tokens as CSS variables. In the test, `colorPalette` classes only re-point variables, for example `.color-palette_teal { --colors-color-palette-1: var(--colors-teal-1); … }`. Every installed palette's variables exist in the CSS once its tokens are defined.
- So, by Panda's mechanism, a deployment could change accent without rebuilding by (a) generating the palette classes ahead of time (`staticCss.css: [{ properties: { colorPalette: [...] } }]`) and setting the class at runtime, or (b) overriding `--colors-<palette>-*` variables in a stylesheet. Both were seen in the generated CSS. Park UI documents neither (**unverified** end to end in a browser).
- The live theming page (park-ui.com/docs/theming) does not mention dark-mode switching, fonts, high contrast, density or runtime theme switching.

## 5. Dark mode, high contrast, density

- **Dark mode:** semantic tokens carry `_light` and `_dark`. Park UI adds `light: ':root &, .light &'`. Panda 2.4's `preset-base` defines `dark: '.dark &'`; the generated CSS has `.dark { --colors-amber-1: … }`. Our design system uses `[data-theme=dark]`. An old issue (#312, 2024) shows users overriding `dark` and `light` conditions to use `data-theme`.
- **System preference:** not automatic; issue #312 asked for `prefers-color-scheme` and the answer was to set the attribute from script.
- **High contrast:** none. No condition, no third token value. Radix Colors publishes no high-contrast scale (**unverified** beyond the Radix docs page read).
- **Density:** none. Size is a per-component variant, not a global mode.

## 6. Accessibility

- Park UI's claims are general: "beautifully-designed, accessible components" (introduction); accessibility "from Ark UI" (blog post). No contrast figures, no WCAG conformance statement, no audit.
- Radix Colors ("Understanding the scale"): steps 11 and 12 "are guaranteed to Lc 60 and Lc 90 APCA contrast ratio on top of a step 2 background". Most step 9 colours are made for white text; sky, mint, lime, yellow and amber need dark text. Park UI's `solid.fg` is `white` for blue (read for blue only; **unverified** for the five light hues).
- Disabled state: `layerStyles.disabled` sets `opacity: 0.67` and `filter: grayscale(100%)`.
- Storybook in the repo includes `@storybook/addon-a11y`. No CI accessibility test was seen.
- Known issues in the tracker: switch focus outline missing (#457, closed 2024); contrast between canvas and default backgrounds (#25, closed 2023). No open accessibility issue found.
- The checkbox recipe warning in §3.3 means one style block never applies under Panda 2 (**unverified** whether that is a focus or contrast style).

## 7. Components

React components in `components/react/src/components/ui` (62): absolute-center, accordion, alert, avatar, badge, breadcrumb, button, card, carousel, checkbox, clipboard, close-button, code, collapsible, color-picker, combobox, date-picker, dialog, display-value, drawer, editable, field, fieldset, file-upload, group, heading, hover-card, icon-button, icon, image, input-addon, input-group, input, kbd, link, loader, menu, number-input, pagination, pin-input, popover, progress, radio-card-group, radio-group, rating-group, scroll-area, segment-group, select, skeleton, slider, span, spinner, splitter, switch, table, tabs, tags-input, text, textarea, toast, toggle-group, tooltip. The preset also has recipes without a React component yet: angle-slider, floating-panel, listbox, qr-code, signature-pad, steps, timer, tour, tree-view.

| Need | Ours | Park UI |
|---|---|---|
| Field | yes | yes (`field`, `fieldset`) |
| Select | yes | yes |
| Combobox | yes | yes |
| Dialog | yes | yes (plus `drawer`) |
| Menu | yes | yes |
| Toast | yes | yes |
| Date Picker | yes | yes (component added after the 2025-11 release) |
| DataTable (grid keyboard, sort, virtual) | yes | no; `table` recipe only |
| Sidebar / app shell | no | no |
| Tabs | no | yes |
| Card | no | yes |
| Badge | no | yes |
| Avatar | no | yes |
| Skeleton | no | yes |
| Empty state | no | no |

Our components localize the accessible names of composed parts (close buttons, calendar). Whether Park UI's components hard-code English labels is **unverified**.

## 8. Alternatives

### 8.1 Chakra UI 3 recipes

- `@chakra-ui/react` 3.38.0 (2026-11-03), MIT. Depends on `@ark-ui/react` 5.40.0 and Emotion (`@emotion/react >=11` peer). Its components run on Chakra's own styling runtime, not Panda.
- **`@chakra-ui/panda-preset`** 3.38.0 (2026-11-03), MIT, from `packages/panda-preset` in the Chakra repo. README: a "Panda CSS preset that mirrors the Chakra UI default theme (tokens, semantic colors, recipes, slot recipes, and global styles)". Dark mode by `class="dark"` on `<html>`. Depends on `@pandacss/types ^1.4.2`.
- Build test: with `@pandacss/dev` 2.4.0, `presets: ['@chakra-ui/panda-preset']` and `staticCss.recipes: '*'`, `codegen` and `cssgen` succeeded (223 KB CSS) with seven `nested_property` warnings (segment group, select, slider, switch, tags input). It generated 76 recipe modules, including `empty-state`, `data-list`, `stat`, `status`, `tree-view`, `listbox`, `steps`, `timeline`, `action-bar`, `tag`, plus all the ones in §7. The recipe set is named `swittch` (sic).
- It gives recipes only. Wiring them to Ark parts is our work, as with Park UI.
- Chakra colours are a 50–950 scale with semantic `colorPalette` slots (`solid`, `contrast`, `fg`, `muted`, `subtle`, `emphasized`, `focusRing`) (**unverified** in detail; seen in recipe names only).
- The Panda repo has a design note "Chakra UI as a Panda v2 Design System" (`design-notes/chakra-ui-design-system-migration.md`, branch `v2`) proposing an `@chakra-ui/styled-system` package and removing Emotion. It is a plan, not a release; `@chakra-ui/styled-system` on npm is still 2.12.0 (v2 line).

### 8.2 Copying Park UI recipes as source

- This is what the CLI does since v1, so it is the vendor's model, not a fork.
- The source is MIT. Recipes are plain `defineRecipe` and `defineSlotRecipe` objects that built under Panda 2.4 (§3.3). Component wrappers need the `createStyleContext` rename.
- Our recipes already sit in `packages/design-system/src/theme/recipes` and use our own conditions (`dark`, `contrast`, `compact` on `data-theme` and `data-density`). Park UI recipes use `_light`/`_dark` tokens, `colorPalette.*` slots and size variants, so adopting them would mean adding a high-contrast value to each colour token and a density mapping (**unverified** effort).

## 9. Not verified

- Whether `@park-ui/cli` 1.0.1 runs with TypeScript 7 and a Panda 2 config.
- Runtime behaviour of the Park UI components after the `createStyleContext` rename (only type-checked).
- What the checkbox `control.borderColor` style was meant to do and its effect under Panda 2.
- Runtime accent switching end to end in a browser (seen only in generated CSS).
- `solid.fg` text colour for Radix's light hues (sky, mint, lime, yellow, amber).
- Whether Park UI components hard-code English accessible names.
- Whether a Panda 2 migration of Park UI is planned (issue #577 unanswered).
- Chakra's semantic palette slots and its v2 styled-system plan timing.

## 10. Sources (read 2026-11-18)

1. npm registry: `npm view` for `@park-ui/panda-preset`, `@park-ui/cli`, `@park-ui/tailwind-plugin`, `@pandacss/dev`, `@ark-ui/react`, `@chakra-ui/react`, `@chakra-ui/panda-preset`, `@chakra-ui/styled-system` (versions, `time`, peers, licence); `npm search park-ui`.
2. GitHub `chakra-ui/park-ui` (formerly `cschroeter/park-ui`), `main` at `102769cc91a9d36753dbc5893c74b7954b495efe` (2026-02-21): `README.md`, `components/react/package.json`, `components/react/panda.config.ts`, `components/react/src/components/ui/*`, `packages/preset/package.json`, `packages/preset/src/index.ts`, `packages/preset/src/theme/*`, `packages/preset/src/recipes/*`, `packages/cli/README.md`, `packages/cli/src/commands/init.ts`, `packages/cli/src/utils/prompt.ts`, `packages/cli/src/utils/registry-client.ts`, `website/public/registry/react/*.json`. https://github.com/chakra-ui/park-ui
3. Park UI docs source: `website/src/content/docs/theming.mdx`, `installation.mdx`, `migration.mdx`, `introduction.mdx`; changelog `website/src/content/changelog/2025-11-21.mdx`; posts `park-ui-joins-the-chakra-ui-organization.mdx` (2025-11-27) and `why-choose-park-ui-for-your-design-system.mdx`.
4. Park UI theming page (live). https://park-ui.com/docs/theming
5. Park UI issues: #577, #312, #457, #25 and the open issue list. https://github.com/chakra-ui/park-ui/issues
6. GitHub API: repository metadata, branches, commits and releases of `chakra-ui/park-ui`.
7. Panda CSS, "Panda CSS 2.0" blog post (2026-09-29), `website/content/blog/panda-css-v2.mdx`, branch `v2`. https://github.com/chakra-ui/panda ; upgrade guide https://panda-css.com/docs/get-started/upgrading-to-v2 (linked, not read).
8. Panda CSS design note `design-notes/chakra-ui-design-system-migration.md`, branch `v2`.
9. `@pandacss/preset-base` 2.4.0 `dist/index.js` (`dark: ".dark &"`), installed from npm.
10. Chakra UI: `packages/react/src/theme/recipes/` (directory listing, `empty-state.ts`) and `packages/panda-preset/README.md`. https://github.com/chakra-ui/chakra-ui
11. Radix Colors, "Understanding the scale". https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale
12. Build tests run in a scratch directory with Bun 1.4.2 and Node 22.16 (§3.3, §8.1).
13. This repository: `packages/design-system/package.json`, `src/theme/conditions.ts`, `src/theme/semantic-tokens.ts`, `docs/architecture/client-application.md` § Design system, `docs/research/frontend.md`.
