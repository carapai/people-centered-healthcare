// A Report as the Report designer edits it (studio.md § Reports; analytics.md § Reports; ADR 0070,
// ADR 0072; #559): `configuration/reports/<id>.json` in a Reporting package, written to the
// release's `report/1` schema and read on the server as `health_analytics::ReportDefinition`.
// This module reads the file, edits it one step at a time and writes it back in the schema's key
// order, so a Report built here is the same definition as one written by hand. Validation is the
// server's (`POST …/reports/preview` answers its problems as the author edits); nothing here
// decides what is valid.

export const REPORT_SCHEMA = 'https://carapai.github.io/people-centered-healthcare/fhir/schema/report/1'
export const REPORTS_FOLDER = 'configuration/reports/'

export const PERIOD_TYPES = ['weekly', 'monthly', 'quarterly', 'semiAnnual', 'annual', 'financialJuly'] as const
export type PeriodType = (typeof PERIOD_TYPES)[number]

export const LEVELS = ['facility', 'district', 'region', 'national'] as const
export type Level = (typeof LEVELS)[number]

export type Axis = 'rows' | 'columns'

/** One of a table section's Disaggregations: an Age Band Set (`url|version`) or a coded value. */
export type SectionDisaggregation = { ageBandSet: string; axis: Axis } | { code: string; expression: string; axis: Axis }

export interface IndicatorRow {
  id: string
  /** `url|version`. */
  indicator: string
  label: string
}

export interface TableSection {
  kind: 'table'
  id: string
  title: string
  disaggregations: SectionDisaggregation[]
  rows: IndicatorRow[]
  columnTotals: boolean
  rowTotals: boolean
  change: boolean
  sendToDhis2: boolean
}

export interface Code {
  system: string
  code: string
}

export const PICKS = ['latest', 'first', 'all', 'count'] as const
export type ListPick = (typeof PICKS)[number]

export interface ListColumn {
  id: string
  label: string
  attribute?: string
  concept?: Code
  pick?: ListPick
}

export interface LineListSection {
  kind: 'lineList'
  id: string
  title: string
  rowPer: 'enrollment' | 'visit'
  program: string
  enrollments?: 'started' | 'active'
  visitTypes?: Code[]
  countedBy?: string
  columns: ListColumn[]
}

export type Section = TableSection | LineListSection

export interface Report {
  id: string
  title: string
  periodType: PeriodType
  levels: Level[]
  sections: Section[]
}

/** The attributes a line list column may show (`health_analytics::report::ATTRIBUTES`), and
 * what each is of. */
export const ATTRIBUTES = [
  ['person.name', 'person'],
  ['person.sex', 'person'],
  ['person.birthDate', 'person'],
  ['person.age', 'person'],
  ['person.phone', 'person'],
  ['person.address', 'person'],
  ['person.identifier', 'person'],
  ['enrollment.start', 'enrollment'],
  ['enrollment.end', 'enrollment'],
  ['enrollment.status', 'enrollment'],
  ['enrollment.visits', 'enrollment'],
  ['enrollment.lastVisit', 'enrollment'],
  ['visit.date', 'visit'],
  ['visit.type', 'visit'],
  ['visit.facility', 'visit'],
  ['visit.recorded', 'visit'],
] as const

/** The attributes a column may show on a list of `rowPer`: a visit's only on a visit list. */
export const attributesFor = (rowPer: LineListSection['rowPer']) => ATTRIBUTES.filter(([, of]) => of !== 'visit' || rowPer === 'visit').map(([name]) => name)

/** An id from a title: lower case, words joined by `-`. */
export function slug(text: string): string {
  return (
    text
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'new'
  )
}

/** `wanted`, or `wanted-2`, `wanted-3`… if `taken` has it. */
export function unique(wanted: string, taken: readonly string[]): string {
  let id = wanted
  for (let n = 2; taken.includes(id); n += 1) id = `${wanted}-${n}`
  return id
}

/** A new Report: monthly, at facility level, with no sections yet. */
export const newReport = (id: string, title: string): Report => ({ id, title, periodType: 'monthly', levels: ['facility'], sections: [] })

/** Where Report `id` lives. */
export const reportPath = (id: string) => `${REPORTS_FOLDER}${id}.json`

const str = (v: unknown) => (typeof v === 'string' ? v : '')
const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v)

/** The Report in `text`, or why the designer cannot edit it. */
export function readReport(text: string): { report: Report } | { refusal: string } {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return { refusal: 'not JSON' }
  }
  if (!isObject(value)) return { refusal: 'not a JSON object' }
  if (value['$schema'] !== REPORT_SCHEMA) return { refusal: 'not written to the report/1 schema' }
  if (!Array.isArray(value['sections']) || !value['sections'].every((s) => isObject(s) && (s['kind'] === 'table' || s['kind'] === 'lineList'))) return { refusal: 'a section is neither a table nor a line list' }
  const report: Report = {
    id: str(value['id']),
    title: str(value['title']),
    periodType: PERIOD_TYPES.includes(value['periodType'] as PeriodType) ? (value['periodType'] as PeriodType) : 'monthly',
    levels: Array.isArray(value['levels']) ? (value['levels'] as unknown[]).filter((l): l is Level => LEVELS.includes(l as Level)) : [],
    sections: (value['sections'] as Record<string, unknown>[]).map((s): Section => {
      if (s['kind'] === 'table')
        return {
          kind: 'table',
          id: str(s['id']),
          title: str(s['title']),
          disaggregations: (Array.isArray(s['disaggregations']) ? (s['disaggregations'] as unknown[]) : []).filter(isObject).map((d): SectionDisaggregation => {
            const axis: Axis = d['axis'] === 'rows' ? 'rows' : 'columns'
            return typeof d['ageBandSet'] === 'string' ? { ageBandSet: d['ageBandSet'], axis } : { code: str(d['code']), expression: str(d['expression']), axis }
          }),
          rows: (Array.isArray(s['rows']) ? (s['rows'] as unknown[]) : []).filter(isObject).map((r) => ({ id: str(r['id']), indicator: str(r['indicator']), label: str(r['label']) })),
          columnTotals: s['columnTotals'] === true,
          rowTotals: s['rowTotals'] === true,
          change: s['change'] === true,
          sendToDhis2: s['sendToDhis2'] === true,
        }
      const list: LineListSection = {
        kind: 'lineList',
        id: str(s['id']),
        title: str(s['title']),
        rowPer: s['rowPer'] === 'visit' ? 'visit' : 'enrollment',
        program: str(s['program']),
        columns: (Array.isArray(s['columns']) ? (s['columns'] as unknown[]) : []).filter(isObject).map((c) => {
          const column: ListColumn = { id: str(c['id']), label: str(c['label']) }
          if (typeof c['attribute'] === 'string') column.attribute = c['attribute']
          if (isObject(c['concept'])) column.concept = { system: str(c['concept']['system']), code: str(c['concept']['code']) }
          if (PICKS.includes(c['pick'] as ListPick)) column.pick = c['pick'] as ListPick
          return column
        }),
      }
      if (s['enrollments'] === 'started' || s['enrollments'] === 'active') list.enrollments = s['enrollments']
      if (Array.isArray(s['visitTypes'])) list.visitTypes = (s['visitTypes'] as unknown[]).filter(isObject).map((c) => ({ system: str(c['system']), code: str(c['code']) }))
      if (typeof s['countedBy'] === 'string') list.countedBy = s['countedBy']
      return list
    }),
  }
  return { report }
}

/** The file for `report`, in the schema's key order. */
export function writeReport(report: Report): Record<string, unknown> {
  return {
    $schema: REPORT_SCHEMA,
    id: report.id,
    title: report.title,
    periodType: report.periodType,
    levels: LEVELS.filter((l) => report.levels.includes(l)),
    sections: report.sections.map((s) => {
      if (s.kind === 'table')
        return {
          kind: 'table',
          id: s.id,
          title: s.title,
          disaggregations: s.disaggregations.map((d) => ('ageBandSet' in d ? { ageBandSet: d.ageBandSet, axis: d.axis } : { code: d.code, expression: d.expression, axis: d.axis })),
          rows: s.rows.map((r) => ({ id: r.id, indicator: r.indicator, label: r.label })),
          columnTotals: s.columnTotals,
          rowTotals: s.rowTotals,
          change: s.change,
          sendToDhis2: s.sendToDhis2,
        }
      return {
        kind: 'lineList',
        id: s.id,
        title: s.title,
        rowPer: s.rowPer,
        program: s.program,
        ...(s.rowPer === 'enrollment' && s.enrollments !== undefined ? { enrollments: s.enrollments } : {}),
        ...(s.rowPer === 'visit' && (s.visitTypes ?? []).length > 0 ? { visitTypes: s.visitTypes } : {}),
        ...(s.countedBy === undefined || s.countedBy === '' ? {} : { countedBy: s.countedBy }),
        columns: s.columns.map((c) => ({
          id: c.id,
          label: c.label,
          ...(c.attribute === undefined ? {} : { attribute: c.attribute }),
          ...(c.concept === undefined ? {} : { concept: c.concept }),
          ...(c.pick === undefined ? {} : { pick: c.pick }),
        })),
      }
    }),
  }
}

// ---------------------------------------------------------------------------------------------
// Edits. Each returns a new Report.

const replaceAt = <T>(list: readonly T[], at: number, next: T): T[] => list.map((x, n) => (n === at ? next : x))

/** Moves item `at` of `list` by `by` (−1 up, +1 down), if it can move. */
export function move<T>(list: readonly T[], at: number, by: -1 | 1): T[] {
  const to = at + by
  if (to < 0 || to >= list.length) return [...list]
  const next = [...list]
  const [item] = next.splice(at, 1)
  next.splice(to, 0, item as T)
  return next
}

export const withSettings = (report: Report, next: Partial<Pick<Report, 'title' | 'periodType' | 'levels'>>): Report => ({ ...report, ...next })

export function toggleLevel(report: Report, level: Level, on: boolean): Report {
  const levels = on ? [...report.levels.filter((l) => l !== level), level] : report.levels.filter((l) => l !== level)
  return { ...report, levels: LEVELS.filter((l) => levels.includes(l)) }
}

/** Adds an empty table section titled `title`; its id comes from the title. */
export function addTable(report: Report, title: string): Report {
  const id = unique(slug(title), report.sections.map((s) => s.id))
  const section: TableSection = { kind: 'table', id, title, disaggregations: [], rows: [], columnTotals: false, rowTotals: false, change: false, sendToDhis2: false }
  return { ...report, sections: [...report.sections, section] }
}

/** Adds a line list titled `title`: one row per enrollment active during the period. */
export function addLineList(report: Report, title: string): Report {
  const id = unique(slug(title), report.sections.map((s) => s.id))
  const section: LineListSection = { kind: 'lineList', id, title, rowPer: 'enrollment', program: '', enrollments: 'active', columns: [] }
  return { ...report, sections: [...report.sections, section] }
}

export const removeSection = (report: Report, at: number): Report => ({ ...report, sections: report.sections.filter((_, n) => n !== at) })
export const moveSection = (report: Report, at: number, by: -1 | 1): Report => ({ ...report, sections: move(report.sections, at, by) })

/** Section `at` changed by `change`, if it is of the kind `change` takes. */
export function updateSection<K extends Section['kind']>(report: Report, at: number, kind: K, change: (section: Extract<Section, { kind: K }>) => Extract<Section, { kind: K }>): Report {
  const section = report.sections[at]
  if (section?.kind !== kind) return report
  return { ...report, sections: replaceAt(report.sections, at, change(section as Extract<Section, { kind: K }>)) }
}

export const withSection = (report: Report, at: number, next: Partial<Pick<Section, 'id' | 'title'>>): Report => {
  const section = report.sections[at]
  return section === undefined ? report : { ...report, sections: replaceAt(report.sections, at, { ...section, ...next } as Section) }
}

// Table sections.

export const addAgeBands = (s: TableSection, ageBandSet: string, axis: Axis = 'columns'): TableSection => ({ ...s, disaggregations: [...s.disaggregations, { ageBandSet, axis }] })
export const addCoded = (s: TableSection, code: string, expression: string, axis: Axis = 'columns'): TableSection => ({ ...s, disaggregations: [...s.disaggregations, { code, expression, axis }] })
export const withDisaggregation = (s: TableSection, at: number, next: SectionDisaggregation): TableSection => ({ ...s, disaggregations: replaceAt(s.disaggregations, at, next) })
export const removeDisaggregation = (s: TableSection, at: number): TableSection => ({ ...s, disaggregations: s.disaggregations.filter((_, n) => n !== at) })
export const withOptions = (s: TableSection, next: Partial<Pick<TableSection, 'columnTotals' | 'rowTotals' | 'change' | 'sendToDhis2'>>): TableSection => ({ ...s, ...next })

/** The name a Disaggregation is known by: its Age Band Set or its code. */
export const disaggregationName = (d: SectionDisaggregation) => ('ageBandSet' in d ? d.ageBandSet : d.code)

/** Adds a row for `indicator` (`url|version`) labelled `label`; its id comes from the
 * indicator's own id unless `id` is given. */
export function addRow(s: TableSection, indicator: string, label: string, id?: string): TableSection {
  const own = indicator.split('|')[0]?.split('/').pop() ?? ''
  const rowId = unique(id ?? slug(own || label), s.rows.map((r) => r.id))
  return { ...s, rows: [...s.rows, { id: rowId, indicator, label }] }
}

export const withRow = (s: TableSection, at: number, next: Partial<IndicatorRow>): TableSection => ({ ...s, rows: s.rows.map((r, n) => (n === at ? { ...r, ...next } : r)) })
export const removeRow = (s: TableSection, at: number): TableSection => ({ ...s, rows: s.rows.filter((_, n) => n !== at) })
export const moveRow = (s: TableSection, at: number, by: -1 | 1): TableSection => ({ ...s, rows: move(s.rows, at, by) })

// Line list sections.

/** One row per enrollment (started in, or active during, the period) or per visit: switching
 * drops what the other kind of row needs. */
export function withRowPer(s: LineListSection, rowPer: LineListSection['rowPer']): LineListSection {
  if (rowPer === s.rowPer) return s
  const columns = s.columns
    .filter((c) => rowPer === 'visit' || !c.attribute?.startsWith('visit.'))
    .map((c) => {
      if (c.concept === undefined) return c
      const { pick: _pick, ...rest } = c
      return rowPer === 'visit' ? rest : { ...rest, pick: 'latest' as const }
    })
  const { enrollments: _e, visitTypes: _v, ...rest } = s
  return rowPer === 'visit' ? { ...rest, rowPer, visitTypes: [], columns } : { ...rest, rowPer, enrollments: 'active', columns }
}

export const withList = (s: LineListSection, next: Partial<Pick<LineListSection, 'program' | 'enrollments' | 'countedBy'>>): LineListSection => {
  const merged = { ...s, ...next }
  if (merged.countedBy === '') delete merged.countedBy
  return merged
}

export const addVisitType = (s: LineListSection, code: Code): LineListSection => ({ ...s, visitTypes: [...(s.visitTypes ?? []), code] })
export const withVisitType = (s: LineListSection, at: number, code: Code): LineListSection => ({ ...s, visitTypes: replaceAt(s.visitTypes ?? [], at, code) })
export const removeVisitType = (s: LineListSection, at: number): LineListSection => ({ ...s, visitTypes: (s.visitTypes ?? []).filter((_, n) => n !== at) })

/** Adds an attribute column, labelled `label`; its id comes from the attribute. */
export function addAttribute(s: LineListSection, attribute: string, label: string, id?: string): LineListSection {
  const columnId = unique(id ?? slug(attribute.split('.').pop() ?? attribute), s.columns.map((c) => c.id))
  return { ...s, columns: [...s.columns, { id: columnId, label, attribute }] }
}

/** Adds a concept column, labelled `label`; on an enrollment list it shows the latest value
 * unless `pick` says otherwise. */
export function addConcept(s: LineListSection, concept: Code, label: string, id?: string, pick?: ListPick): LineListSection {
  const columnId = unique(id ?? slug(label), s.columns.map((c) => c.id))
  const column: ListColumn = { id: columnId, label, concept }
  if (s.rowPer === 'enrollment') column.pick = pick ?? 'latest'
  return { ...s, columns: [...s.columns, column] }
}

export const withColumn = (s: LineListSection, at: number, next: Partial<ListColumn>): LineListSection => ({ ...s, columns: s.columns.map((c, n) => (n === at ? { ...c, ...next } : c)) })
export const removeColumn = (s: LineListSection, at: number): LineListSection => ({ ...s, columns: s.columns.filter((_, n) => n !== at) })
export const moveColumn = (s: LineListSection, at: number, by: -1 | 1): LineListSection => ({ ...s, columns: move(s.columns, at, by) })

/** The schema's example period for a type, to prompt the author. */
export const PERIOD_EXAMPLES: Record<PeriodType, string> = { weekly: '2026W10', monthly: '2026-03', quarterly: '2026Q1', semiAnnual: '2026S1', annual: '2026', financialJuly: '2025July' }

/** A one-line summary of a closed section. */
export function summaryOf(section: Section): { kind: 'table'; rows: number; by: number; sent: boolean } | { kind: 'lineList'; rowPer: LineListSection['rowPer']; columns: number } {
  return section.kind === 'table' ? { kind: 'table', rows: section.rows.length, by: section.disaggregations.length, sent: section.sendToDhis2 } : { kind: 'lineList', rowPer: section.rowPer, columns: section.columns.length }
}
