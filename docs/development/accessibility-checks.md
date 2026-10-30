# Accessibility checks

The client targets WCAG 2.2 AA (client-application.md § Design system). Automated checks run on every PR. Manual screen-reader checks run on Windows with NVDA, before a component is first used in a screen and after any change to its keyboard model or ARIA. This page records both.

## Automated: axe

`packages/design-system/test/gallery.test.tsx` runs axe-core 4.14 in Chromium (Vitest browser mode) over the example page, in all six theme × density sections. It uses the tags `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa` and `wcag22aa`, and it fails on any violation. It also fails if axe did not evaluate colour contrast.

| Date | Components | Result |
|---|---|---|
| 2026-10-30 | Field, Select, Combobox, Dialog, Menu, Toast, Date Picker (#45) | No violations. Two found and fixed first: a listbox containing Ark's `List` wrapper (aria-required-children), and the calendar heading button's name missing its visible text (label-content-name-mismatch). |
| 2026-10-30 | DataTable (#46), with the components above | No violations. |

## Manual: NVDA

Run with the latest NVDA and Chrome or Edge on Windows, against `bun run example` in `packages/design-system`, in browse mode and focus mode.

### DataTable script

1. Tab from the Columns button into the grid. NVDA announces the grid's name ("Patients"), "table with 201 rows and 5 columns" (or similar), and the focused column header with its sort state.
2. Press Down Arrow. NVDA reads the cell, and its row and column headers when they change.
3. Press Ctrl+End, then Ctrl+Home. NVDA announces row 201 and then row 1, and reads no intermediate rows.
4. Press PageDown twice. The row number NVDA announces jumps by about one screen.
5. Focus the Age header and press Enter three times. The header is announced as sorted ascending, then descending, then not sorted. The status message ("Sorted by Age, ascending" and so on) is read once each time.
6. Open Columns, toggle Phone off and back on. The menu items are announced as checkable and checked or not checked, and the column count changes.
7. Scroll the grid with the mouse far from the focused row, then press Shift+Tab and Tab. Focus returns to the same cell.
8. Switch to the compact density and to high contrast (`data-density`, `data-theme`), and repeat steps 1 and 2.

### Results

| Date | Component | NVDA / browser | Tester | Result |
|---|---|---|---|---|
| — | DataTable | — | — | Not yet run: needs a Windows machine ([#180](https://github.com/carapai/people-centered-healthcare/issues/180)). |
