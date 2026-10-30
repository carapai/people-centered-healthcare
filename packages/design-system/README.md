# @health/design-system

The design system of the one React app (client-application.md § Design system): a Panda CSS 2 preset built with `panda lib`, slot recipes for Ark UI 5 components, and ICU messages for every string the components show.

```sh
bun run lib            # panda codegen, then panda lib: dist/panda/ and the package.json exports
bun run example        # the example page: every recipe in every theme and density
bun run test           # component and axe tests in Chromium (bunx playwright install chromium once)
bun run check          # lib, typecheck, lint, test, example build
```

## Layout

- `src/theme/`: the conditions (`data-theme`, `data-density`), semantic tokens and slot recipes. `panda.config.ts` assembles them on top of `@pandacss/preset-base` and `@pandacss/preset-panda`.
- `src/components/`: Field, Select, Combobox, Dialog, Menu, Toast and Date Picker. Each part is the Ark part with its slot class (`slot.tsx`). Composed parts, such as close buttons, indicators and the calendar, take their accessible names from the catalog.
- `src/data-table/`: the DataTable, an ARIA grid over TanStack Table 9 and Virtual 3, with its keyboard model, sorting and a Columns menu. `view.ts` maps its view (sort and hidden columns) to URL search parameters and back.
- `src/i18n/`: `messages` (react-intl descriptors with ids `ds.*`), `englishMessages` (the English catalog) and `I18nProvider` (react-intl plus Ark's locale).
- `example/`: the example page, whose sections set each theme and density and render every recipe with the overlays open.
- `test/`: Vitest browser mode in Chromium. It runs the component tests, axe (WCAG 2.2 AA tags) over the example page, checks that themes and densities change the tokens, and checks that every message is valid ICU.

## Using it

The client sets `designSystem: '@health/design-system'` in its Panda config and imports components from `@health/design-system` and `css` from `@health/design-system/css`. Wrap the app in `I18nProvider` with a catalog that merges `englishMessages` with the app's own. Set `data-theme` and `data-density` on `<html>`, or on any subtree.

A DataTable's view belongs to the caller. A route keeps it in its search parameters with `viewToSearch` and `viewFromSearch`. Columns without a `sortValue` don't sort, and columns with `canHide: false` stay visible. Manual NVDA results are in `docs/development/accessibility-checks.md`.

In a Select or Combobox, items go straight into `Content`, which is the listbox. Use `List` only when `Content` also holds something else, such as a search input.
