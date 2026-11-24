import type { Transport } from '../../platform/types'
import { layoutTable, type At, type Cell, type SectionPreview } from '../studio/reports/layout'
import { REPORT_SCHEMA, readReport, type Level, type PeriodType, type Report, type TableSection } from '../studio/reports/report'

// Running a Report (analytics.md § Reports; ADR 0070, ADR 0072; #557): the Reports the session
// may run, each with the places it may be run for, and a Report run for a place and a period,
// read from stored Indicator Reports. At central for the places in the session's area, on an edge
// for its facility, offline. The screen and the CSV export are both drawn from `sectionGrid`, so
// they hold the same cells.

/** A place a Report may be run for. */
export interface ReportPlace {
  code: string
  name: string
  level: Level
  parent: string | null
}

/** A Report the session may run (`GET /api/analytics/reports`). */
export interface ListedReport {
  /** `<package>:<report id>`. */
  id: string
  title: string
  /** `name@version`. */
  package: string
  periodType: PeriodType
  levels: Level[]
  /** The Report as written (`report/1`). */
  definition: Report
  /** The places it may be run for here, in the session's area. */
  places: ReportPlace[]
}

export interface ReportsList {
  where: 'central' | 'facility'
  reports: ListedReport[]
}

/** A missing facility, with its last sync. */
export interface MissingFacility {
  facility: string
  name: string
  lastSync: string | null
}

/** One table row's run. */
export interface RunRow {
  row: string
  indicator: { title: string; calculation: string } | null
  current: SectionPreview['rows'][number]['current']
  previous: SectionPreview['rows'][number]['previous']
  /** A median or distinct count over more than one facility: its aggregate. */
  facilityOnly?: string | null
  /** The Population Estimate its denominator is (#553). */
  estimate?: { type: string; title: string; value: string | null } | null
  error: string | null
}

export type RunSection = { kind: 'table'; disaggregations: SectionPreview['disaggregations']; rows: RunRow[] } | { kind: 'lineList'; rows: null }

/** A Report run (`POST /api/analytics/reports/run`). */
export interface ReportRun {
  where: 'central' | 'facility'
  report: { id: string; title: string; package: string }
  place: { code: string; name: string; level: Level }
  period: { id: string; start: string; end: string }
  previous: { id: string; start: string; end: string } | null
  completeness: { reported: number; expected: number; computedAt: string | null; missing: MissingFacility[] }
  sections: RunSection[]
}

/** What to run. */
export interface RunAsked {
  report: string
  place: string
  period: string
}

/** The server could not answer: the status and why. */
export class ReportsError extends Error {
  constructor(
    readonly status: number,
    readonly detail: string | undefined,
  ) {
    super(`the Reports answered ${status}`)
    this.name = 'ReportsError'
  }
}

function detailOf(body: unknown): string | undefined {
  return typeof body === 'object' && body !== null && 'detail' in body && typeof body.detail === 'string' ? body.detail : undefined
}

/** The Reports the session may run. */
export function reportsQuery(transport: Transport) {
  return {
    queryKey: ['analytics', 'reports'],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<ReportsList> => {
      const response = await transport.request({ method: 'GET', path: '/analytics/reports', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new ReportsError(response.status, detailOf(response.body))
      const list = response.body as ReportsList
      // Each definition read as the designer reads a Report, its defaults filled in.
      const reports = list.reports.flatMap((r) => {
        const read = readReport(JSON.stringify({ ...r.definition, $schema: REPORT_SCHEMA }))
        return 'report' in read ? [{ ...r, definition: read.report }] : []
      })
      return { ...list, reports }
    },
  }
}

/** `asked` run, asked again whenever it changes. */
export function runQuery(transport: Transport, asked: RunAsked) {
  return {
    queryKey: ['analytics', 'reports', 'run', asked],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<ReportRun> => {
      const response = await transport.request({ method: 'POST', path: '/analytics/reports/run', body: asked, ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new ReportsError(response.status, detailOf(response.body))
      const run = response.body as ReportRun
      // Counts arrive as exact JSON numbers (lossless-json): plain numbers here.
      const c = run.completeness
      return { ...run, completeness: { ...c, reported: Number(c.reported), expected: Number(c.expected) } }
    },
  }
}

const pad = (n: number) => String(n).padStart(2, '0')

/** The id of the period of type `kind` holding `day` (UTC), as the server writes it. */
export function periodOf(kind: PeriodType, day: Date): string {
  const year = day.getUTCFullYear()
  const month = day.getUTCMonth() + 1
  switch (kind) {
    case 'weekly': {
      const date = new Date(Date.UTC(year, day.getUTCMonth(), day.getUTCDate()))
      const weekday = date.getUTCDay() === 0 ? 7 : date.getUTCDay()
      date.setUTCDate(date.getUTCDate() + 4 - weekday)
      const week = Math.ceil(((date.getTime() - Date.UTC(date.getUTCFullYear(), 0, 1)) / 86_400_000 + 1) / 7)
      return `${date.getUTCFullYear()}W${pad(week)}`
    }
    case 'monthly':
      return `${year}-${pad(month)}`
    case 'quarterly':
      return `${year}Q${Math.floor((month - 1) / 3) + 1}`
    case 'semiAnnual':
      return `${year}S${month <= 6 ? 1 : 2}`
    case 'annual':
      return String(year)
    case 'financialJuly':
      return `${month >= 7 ? year : year - 1}July`
  }
}

const STEP_DAYS: Record<PeriodType, number> = { weekly: 7, monthly: 28, quarterly: 85, semiAnnual: 175, annual: 360, financialJuly: 360 }

/**
 * The `count` periods of type `kind` before the one holding `today`, newest first: what a Report
 * may be run for. The current period is not offered, since it is not over; the first is the
 * default.
 */
export function closedPeriods(kind: PeriodType, today: Date, count: number): string[] {
  const current = periodOf(kind, today)
  const ids: string[] = []
  let day = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate()))
  while (ids.length < count) {
    const id = periodOf(kind, day)
    if (id !== current && ids.at(-1) !== id) ids.push(id)
    day = new Date(day.getTime() - STEP_DAYS[kind] * 86_400_000)
  }
  return ids
}

/** The words a grid is written in, so the screen and the CSV read alike. */
export interface GridTexts {
  indicator: string
  total: string
  value: string
  change: string
  /** A cell with no figure, and why. */
  notShown: (cell: Cell) => string
  /** A figure, as `calculation` shows it (`45%` for a percentage). */
  figure: (value: string, calculation: string) => string
}

/** A table section laid out as text: its header, and each body row's head and cells. */
export interface Grid {
  header: string[]
  rows: { head: string; cells: string[] }[]
}

const bandsText = (at: readonly At[]) => at.map((a) => a.label).join(' · ')

/** The text of one cell, as the screen and the CSV show it. */
export function cellText(cell: Cell, calculation: string | undefined, texts: GridTexts): string {
  if (cell.figure === null) return texts.notShown(cell)
  const value = cell.figure.value
  if (value === null) return calculation === 'count' ? String(cell.figure.count) : '—'
  return texts.figure(value, calculation ?? 'count')
}

/** Table section `section` of a Report run at `level`, laid out as text. */
export function sectionGrid(section: TableSection, run: SectionPreview | undefined, level: Level, texts: GridTexts): Grid {
  const layout = layoutTable(section, run, level)
  const header = [texts.indicator, ...layout.columns.map((c) => (c.total ? texts.total : c.bands.length === 0 ? texts.value : bandsText(c.bands)))]
  if (layout.change) header.push(texts.change)
  const rows = layout.rows.map((line) => {
    const found = run?.rows.find((r) => r.row === line.row.id)
    const head = line.total ? `${line.row.label} · ${texts.total}` : line.bands.length === 0 ? line.row.label : `${line.row.label} · ${bandsText(line.bands)}`
    const cells = line.cells.map((cell) => cellText(cell, found?.indicator?.calculation ?? undefined, texts))
    if (line.change !== null) cells.push(line.change.change === null ? '—' : `${line.change.change}%`)
    return { head, cells }
  })
  return { header, rows }
}

/** The run's table sections, each with its section and grid; line lists have none. */
export function runGrids(definition: Report, run: ReportRun, texts: GridTexts): { section: TableSection; grid: Grid }[] {
  return definition.sections.flatMap((section, n) => {
    const shown = run.sections[n]
    if (section.kind !== 'table' || shown?.kind !== 'table') return []
    return [{ section, grid: sectionGrid(section, shown, run.place.level, texts) }]
  })
}

/** One CSV field, quoted when it must be. */
const field = (text: string) => (/[",\n\r]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text)

/**
 * The run as CSV: the Report's title, place and period, then each table section's title, its
 * header and its rows, exactly the cells the screen shows; a blank line between sections.
 */
export function reportCsv(definition: Report, run: ReportRun, texts: GridTexts): string {
  const lines: string[][] = [[run.report.title], [run.place.name, run.period.id]]
  for (const { section, grid } of runGrids(definition, run, texts)) {
    lines.push([], [section.title], grid.header)
    for (const row of grid.rows) lines.push([row.head, ...row.cells])
  }
  return `${lines.map((l) => l.map(field).join(',')).join('\r\n')}\r\n`
}
