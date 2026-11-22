import type { StudioReportBands, StudioReportFigures, StudioReportRow, StudioReportSection } from '@health/api-client'

import { disaggregationName, type IndicatorRow, type Level, type TableSection } from './report'

// How a table section of a Report is laid out from its preview (studio.md § Reports; ADR 0070,
// ADR 0072; #559). Each Disaggregation's bands go on rows or columns; several on one side are
// crossed. Under each indicator come its row bands (and a Total row with column totals); across
// come the column bands (and a Total column with row totals, and % change from the previous
// period). A cell is an indicator at one band of some of the section's Disaggregations, read from
// the indicator's own figure for that band, which central computed with what roll-up needs; a
// Total is the figure over every band of its side, so it is never an addition of percentages.
//
// What a cell cannot show it says, with the reason: an indicator not broken down by one of the
// section's Disaggregations (a note, not an error), a cell crossing two Disaggregations (each
// indicator's figures are per Disaggregation, so crossed cells are computed when Reports run,
// #557), and a median or distinct count above facility level (they cannot be rebuilt from facility
// summaries, ADR 0070).

/** One band of one of the section's Disaggregations. */
export interface At {
  /** The Disaggregation's name: an Age Band Set's `url|version`, or a code. */
  name: string
  code: string
  label: string
}

/** Why a cell shows no figure. */
export type NotShown = 'not-broken-down' | 'crossed' | 'facility-only' | 'no-indicator' | 'no-preview'

export interface Figure {
  value: string | null
  count: number
  denominator: number | null
}

export interface Cell {
  /** The row bands, then the column bands, it is at; none for the whole. */
  at: At[]
  /** Whether it is a Total, over a side's bands. */
  total: boolean
  figure: Figure | null
  previous: Figure | null
  /** Why there is no figure, if there is none. */
  notShown: NotShown | null
  /** For `not-broken-down`, the Disaggregation the indicator lacks. */
  missing?: string
}

export interface ChangeCell {
  /** % change from the previous period, to one decimal place, or null when it cannot be had. */
  change: string | null
}

export interface BodyRow {
  /** The indicator row it is under. */
  row: IndicatorRow
  /** Its index in the section's rows. */
  index: number
  /** Its row bands; none for the indicator's own line. */
  bands: At[]
  /** Whether it is the Total row under the indicator. */
  total: boolean
  cells: Cell[]
  change: ChangeCell | null
}

export interface Column {
  bands: At[]
  total: boolean
}

export interface Layout {
  columns: Column[]
  /** Whether a % change column follows. */
  change: boolean
  rows: BodyRow[]
}

const FACILITY_ONLY = ['median', 'distinct-count']

/** Whether `calculation` can be shown at `level`. */
export const shownAt = (calculation: string | undefined, level: Level) => level === 'facility' || calculation === undefined || !FACILITY_ONLY.includes(calculation)

/** Every combination of one band from each of `by`, in order: the cartesian product. */
function crossed(by: readonly StudioReportBands[]): At[][] {
  return by.reduce<At[][]>((combos, d) => combos.flatMap((combo) => d.bands.map((b) => [...combo, { name: d.name, code: b.code, label: b.label }])), [[]])
}

/** The section's Disaggregations on one side, with their bands from the preview (or none yet). */
function side(section: TableSection, preview: StudioReportSection | undefined, axis: 'rows' | 'columns'): StudioReportBands[] {
  return section.disaggregations
    .filter((d) => d.axis === axis)
    .map((d) => {
      const name = disaggregationName(d)
      return preview?.disaggregations.find((p) => p.name === name) ?? { name, title: name, bands: [] }
    })
}

const asFigure = (f: { value?: { toString: () => string } | null | undefined; count: number; denominator?: number | null | undefined }): Figure => ({
  value: f.value === null || f.value === undefined ? null : f.value.toString(),
  count: f.count,
  denominator: f.denominator ?? null,
})

/** The indicator's figure at `at`, or why it has none. */
export function figureAt(figures: StudioReportFigures | null | undefined, at: readonly At[]): { figure: Figure } | { notShown: NotShown; missing?: string } {
  if (figures === null || figures === undefined) return { notShown: 'no-preview' }
  if (at.length === 0) return { figure: asFigure(figures) }
  const missing = at.find((a) => !figures.disaggregations.some((d) => d.name === a.name))
  if (missing !== undefined) return { notShown: 'not-broken-down', missing: missing.name }
  if (at.length > 1) return { notShown: 'crossed' }
  const [only] = at as [At]
  const band = figures.disaggregations.find((d) => d.name === only.name)?.bands.find((b) => b.code === only.code)
  // A coded value nobody had: nothing was counted in that band.
  return { figure: band === undefined ? { value: null, count: 0, denominator: null } : asFigure(band) }
}

function cellOf(preview: StudioReportRow | undefined, at: At[], total: boolean, level: Level): Cell {
  if (preview === undefined) return { at, total, figure: null, previous: null, notShown: 'no-preview' }
  if (preview.indicator === null || preview.indicator === undefined) return { at, total, figure: null, previous: null, notShown: 'no-indicator' }
  if (!shownAt(preview.indicator.calculation, level)) return { at, total, figure: null, previous: null, notShown: 'facility-only' }
  const now = figureAt(preview.current, at)
  if ('notShown' in now) return { at, total, figure: null, previous: null, notShown: now.notShown, ...(now.missing === undefined ? {} : { missing: now.missing }) }
  const before = figureAt(preview.previous, at)
  return { at, total, figure: now.figure, previous: 'figure' in before ? before.figure : null, notShown: null }
}

/** % change from `previous` to `now`, to one decimal place. */
export function percentChange(now: Figure | null, previous: Figure | null): string | null {
  const a = Number(now?.value ?? Number.NaN)
  const b = Number(previous?.value ?? Number.NaN)
  if (!Number.isFinite(a) || !Number.isFinite(b) || b === 0) return null
  return (((a - b) / b) * 100).toFixed(1)
}

/** The table section `section` laid out at `level`, from its preview if there is one. */
export function layoutTable(section: TableSection, preview: StudioReportSection | undefined, level: Level): Layout {
  const rowSide = side(section, preview, 'rows')
  const columnSide = side(section, preview, 'columns')
  const columnBands = columnSide.length === 0 ? [[]] : crossed(columnSide)
  const columns: Column[] = columnBands.map((bands) => ({ bands, total: false }))
  if (section.rowTotals && columnSide.length > 0) columns.push({ bands: [], total: true })
  const rows: BodyRow[] = []
  section.rows.forEach((row, index) => {
    const found = preview?.rows.find((r) => r.row === row.id)
    const line = (bands: At[], total: boolean): BodyRow => {
      const cells = columns.map((c) => cellOf(found, [...bands, ...c.bands], c.total || total, level))
      const whole = cellOf(found, bands, true, level)
      return { row, index, bands, total, cells, change: section.change ? { change: percentChange(whole.figure, whole.previous) } : null }
    }
    if (rowSide.length === 0) rows.push(line([], false))
    else {
      for (const bands of crossed(rowSide)) rows.push(line(bands, false))
      if (section.columnTotals) rows.push(line([], true))
    }
  })
  return { columns, change: section.change, rows }
}

/** The test patients a row counted at `at`: those with a counted record in every band of `at`. */
export function countedAt(preview: StudioReportRow | undefined, at: readonly At[]): string[] {
  if (preview === undefined) return []
  const names = preview.current?.disaggregations.map((d) => d.name) ?? []
  return preview.counted
    .filter((c) =>
      c.bands.some((record) =>
        at.every((a) => {
          const n = names.indexOf(a.name)
          return n >= 0 && record[n] === a.code
        }),
      ),
    )
    .map((c) => c.name)
}

/** A cell sent to DHIS2: an indicator × row band × column band of a section that is sent. */
export interface SentCell {
  section: number
  sectionTitle: string
  row: IndicatorRow
  at: At[]
  total: boolean
  /** Whether it is a *not recorded* band: sent only if mapped. */
  notRecorded: boolean
}

/** Every cell of the sent sections, in order: what the DHIS2 mapping sheet lists. */
export function sentCells(sections: readonly { section: TableSection; index: number; preview: StudioReportSection | undefined }[]): SentCell[] {
  return sections
    .filter((s) => s.section.sendToDhis2)
    .flatMap(({ section, index, preview }) =>
      layoutTable(section, preview, 'facility').rows.flatMap((line) =>
        line.cells.map((cell) => ({
          section: index,
          sectionTitle: section.title,
          row: line.row,
          at: cell.at,
          total: cell.total,
          notRecorded: cell.at.some((a) => a.code === 'not-recorded'),
        })),
      ),
    )
}
