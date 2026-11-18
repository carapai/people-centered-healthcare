# The design system copies Park UI recipes as source onto Radix Colors

Decided in [How do we adopt Park UI in the design system, and migrate the existing components and client?](https://github.com/carapai/people-centered-healthcare/issues/645), on the map [Wayfinder: a professional, themeable design system (Park UI)](https://github.com/carapai/people-centered-healthcare/issues/643), after the research [Is Park UI current and compatible with Panda CSS 2 and Ark UI 5, and what does its theming offer?](https://github.com/carapai/people-centered-healthcare/issues/644) ([park-ui.md](https://github.com/carapai/people-centered-healthcare/blob/research/park-ui/docs/research/park-ui.md), branch `research/park-ui`). This reverses "Park UI is not used" in [client-application.md](../architecture/client-application.md).

The research found:
- **Packaging:** Park UI's preset package is retired (last release November 2024). Park UI is now a CLI that copies recipes into a project, under MIT.
- **Versions and fit:** it targets Panda 1.8 and Ark UI 5.30. Under our Panda 2.4 its recipes build, and its components need one function rename.
- **Themes and density:** it uses Radix Colors and a `.dark` class, with no high-contrast theme and no density modes.
- **Chakra:** Chakra UI 3's theme is a current Panda preset (MIT) with more recipes.

Our own design system is about 900 lines: 7 recipes and the DataTable. The client calls `css()` with our semantic token names in 39 files.

**The decision:**
- **Copied source, no dependency.**
  - Park UI's recipes and component parts are copied into `@health/design-system` and ported to Panda 2.
  - Gaps (empty state, data list) are filled the same way from Chakra's Panda preset.
  - `THIRD_PARTY.md` keeps both MIT notices and the upstream commit of each copied file.
- **Radix Colors** scales (12 steps plus alpha) replace Panda's default palette.
  - Light and dark map through our `data-theme` conditions, not a `.dark` class.
  - High contrast keeps its own hand-set values.
- **Park UI's semantic token vocabulary**, including the virtual `colorPalette`, becomes ours.
  - The old names (`bg.canvas`, `fg.muted`, `accent.default`, …) stay as aliases until the client no longer uses them.
  - A lint rule flags old names.
- **Density stays system-wide.** Control heights, padding and gaps come from density-aware tokens (44 px comfortable, 32 px compact). Size variants remain only where size carries meaning, such as buttons and avatars.
- **Component set.**
  - Our existing 7 are restyled from Park UI.
  - Added for the shell: Button, IconButton, Input, Textarea, Checkbox, Radio Group, Switch, Tabs, Card, Badge, Avatar, Skeleton, Tooltip, Popover, Drawer, Alert, Spinner, Progress.
  - Ours: EmptyState (from Chakra's recipe), the App Shell, and the DataTable on the new tokens.
- **Migration in green steps:**
  1. tokens and palette with aliases;
  2. each existing component behind its current export and props;
  3. new components and the App Shell;
  4. the client's `css()` calls moved off old names.
- **Checks.**
  - axe in every theme and density, theme-token checks and ICU checks stay green at every step.
  - Playwright screenshot tests of the example page per theme and density are added.
  - Every visible or accessible string in a copied part moves to `ds.*` messages.

## Considered options

- **Depending on Park UI's packages:** the preset is retired, the CLI is barely maintained, and both target Panda 1.
- **Depending on `@chakra-ui/panda-preset`:** current, but its recipes assume Chakra's own conditions and naming. A dependency would tie our themes, density and high contrast to its release cycle.
- **Restyling our recipes by hand in Park UI's style:** keeps full control, but redoes design work that Park UI and Radix have already done well.
- **Keeping our token names everywhere:** every copied recipe would need rewriting, and accent switching through `colorPalette` would be lost.
