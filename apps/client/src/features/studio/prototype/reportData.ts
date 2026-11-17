// PROTOTYPE (#544), throwaway: a Report (ADR 0070) over the indicator prototype's test patients.
// A Report has a title, one period type, the levels it runs for, sections of indicator rows, columns
// from one Disaggregation or two crossed, and (when sent to DHIS2) a mapping on each cell with
// effective dates. Places, facilities and population figures are made up.
import {
  AGE_SETS,
  INITIAL,
  OTHERS,
  PERIOD_TYPES,
  bands,
  compute,
  denOf,
  disaggLabel,
  evaluate,
  type Disagg,
  type Indicator,
  type Node,
  type Patient,
  type Period,
  type PeriodType,
} from './indicatorData'

// --- Indicators the Report can use (published, by url|version) -------------------------------------------

const cond = (input: string, value: string): Node => ({ kind: 'cond', input, op: '=', value })
const firstVisitTree: Node = { kind: 'group', join: 'all', children: [cond('sex', 'female'), cond('type', 'anc-first')] }
const base = OTHERS[1] as Indicator

export const CATALOGUE: Indicator[] = [
  ...OTHERS,
  { ...INITIAL, version: '2.4.0' },
  { ...base, id: 'anc-hiv-pos', title: 'Pregnant women HIV positive at ANC 1st visit', version: '1.1.0', numerator: { tree: { kind: 'group', join: 'all', children: [cond('sex', 'female'), cond('type', 'anc-first'), cond('hiv', 'positive')] } } },
  { ...base, id: 'anc-median-ga', title: 'Median gestational age at ANC 1st visit', version: '1.0.0', calc: 'median', value: { kind: 'input', input: 'ga' }, pick: 'first', numerator: { tree: firstVisitTree } },
  { ...base, id: 'anc-districts', title: 'Districts ANC clients come from', version: '1.0.0', calc: 'distinct', distinctOf: 'district', numerator: { tree: firstVisitTree } },
  { ...base, id: 'anc-coverage', title: 'ANC 1st visit coverage', version: '1.2.0', calc: 'percentage', den: { tree: { kind: 'group', join: 'all', children: [] }, population: 0 }, numerator: { tree: firstVisitTree } },
]
export const PACKAGE_OF: Record<string, string> = { 'anc-visits': 'ug.moh.anc', 'anc-first': 'ug.moh.anc', 'anc-first-12w': 'ug.moh.anc', 'anc-hiv-pos': 'ug.moh.emtct', 'anc-median-ga': 'ug.moh.anc', 'anc-districts': 'ug.moh.anc', 'anc-coverage': 'ug.moh.anc' }
export const indicator = (id: string) => CATALOGUE.find((i) => i.id === id)
export const canonical = (i: Indicator) => `https://health.go.ug/fhir/${PACKAGE_OF[i.id] ?? 'ug.moh.anc'}/Measure/${i.id}|${i.version}`
/** Medians and distinct counts can't be rebuilt from facility summaries (ADR 0070). */
export const facilityOnly = (i: Indicator) => i.calc === 'median' || i.calc === 'distinct'
/** DHIS2 aggregate data elements hold counts. */
export const isCount = (i: Indicator) => i.calc === 'count' || i.calc === 'sum'

// --- Places --------------------------------------------------------------------------------------------

export type Level = 'facility' | 'district' | 'region' | 'national'
export const LEVELS: Level[] = ['facility', 'district', 'region', 'national']
export type Place = { id: string; label: string; level: Level; parent: string | null }
export const PLACES: Place[] = [
  { id: 'ug', label: 'Uganda', level: 'national', parent: null },
  { id: 'central', label: 'Central region', level: 'region', parent: 'ug' },
  { id: 'kampala', label: 'Kampala district', level: 'district', parent: 'central' },
  { id: 'wakiso', label: 'Wakiso district', level: 'district', parent: 'central' },
  { id: 'kisenyi', label: 'Kisenyi HC IV', level: 'facility', parent: 'kampala' },
  { id: 'kawaala', label: 'Kawaala HC III', level: 'facility', parent: 'kampala' },
  { id: 'wakiso-hc4', label: 'Wakiso HC IV', level: 'facility', parent: 'wakiso' },
]
const FACILITY_OF: Record<string, string> = { p1: 'kisenyi', p3: 'kisenyi', p8: 'kisenyi', p10: 'kisenyi', p5: 'kawaala', p9: 'kawaala', p2: 'wakiso-hc4', p4: 'wakiso-hc4', p6: 'wakiso-hc4', p7: 'wakiso-hc4' }
export const facilityOf = (p: Patient) => FACILITY_OF[p.person] ?? ''
export const facilitiesUnder = (id: string): string[] => {
  const place = PLACES.find((x) => x.id === id)
  if (!place) return []
  if (place.level === 'facility') return [place.id]
  return PLACES.filter((x) => x.parent === id).flatMap((x) => facilitiesUnder(x.id))
}
/** Expected pregnancies per year (Population package ug.population.2026, ADR 0071), lowest level given. */
const EXPECTED_PREGNANCIES: Record<string, number> = { kampala: 96, wakiso: 72 }
const share: Record<PeriodType, number> = { weekly: 7 / 365, monthly: 1 / 12, quarterly: 1 / 4, semiAnnual: 1 / 2, annual: 1, financialJuly: 1 }
export function estimate(placeId: string, period: Period): number | null {
  const own = EXPECTED_PREGNANCIES[placeId]
  if (own !== undefined) return own * share[period.type]
  const children = PLACES.filter((x) => x.parent === placeId)
  if (children.length === 0) return null
  const parts = children.map((x) => estimate(x.id, period))
  return parts.some((x) => x === null) ? null : parts.reduce<number>((a, b) => a + (b ?? 0), 0)
}

// --- DHIS2 metadata (made up; the real list comes from the instance at Validation) -------------------------

export const DATA_SETS = [{ id: 'hmis105m', label: 'HMIS 105: Health unit outpatient monthly report (2024)' }]
export const DATA_ELEMENTS = [
  { id: 'AN01a', label: '105-AN01a. ANC 1st visit for women' },
  { id: 'AN01b', label: '105-AN01b. ANC 1st visit for women within 12 weeks' },
  { id: 'AN02', label: '105-AN02. ANC visits (all)' },
  { id: 'AN17', label: '105-AN17. Pregnant women newly testing HIV positive' },
  { id: 'AN01-2019', label: '105-AN01. ANC 1st visit (2019 form, retired)' },
]
export const COCS = [
  { id: 'y10', label: '10-14 Years' },
  { id: 'y15', label: '15-19 Years' },
  { id: 'y20', label: '20-24 Years' },
  { id: 'y25', label: '25 Years and above' },
  { id: 'default', label: 'default' },
]
export type Mapping = { de: string; coc: string; from: string; to: string }

// --- The Report -----------------------------------------------------------------------------------------

export type Row = { id: string; ind: string; label: string }
export type Section = { id: string; title: string; rows: Row[] }
export type Report = {
  title: string
  pkg: string
  version: string
  periodType: PeriodType
  levels: Level[]
  columns: Disagg[]
  total: boolean
  sections: Section[]
  dhis2: string | null
  mappings: Record<string, Mapping[]>
}
export const cellKey = (row: string, col: string) => `${row}::${col}`

const ageMaps = (de: string, from: string, to = ''): [string, Mapping[]][] =>
  [
    ['10–14 years', 'y10'],
    ['15–19 years', 'y15'],
    ['20–24 years', 'y20'],
    ['25+ years', 'y25'],
  ].map(([col = '', coc = '']) => [col, [{ de, coc, from, to }]])

export const INITIAL_REPORT: Report = {
  title: 'HMIS 105 · 2 Antenatal care',
  pkg: 'ug.moh.hmis',
  version: '4.2.0-draft',
  periodType: 'monthly',
  levels: ['facility', 'district', 'national'],
  columns: [{ kind: 'age', set: 'moh-age-anc' }],
  total: true,
  dhis2: 'hmis105m',
  sections: [
    {
      id: 's1',
      title: '2.1 ANC attendance',
      rows: [
        { id: 'r1', ind: 'anc-first', label: 'ANC 1st visit for women' },
        { id: 'r2', ind: 'anc-first-12w', label: 'ANC 1st visit within 12 weeks (%)' },
        { id: 'r3', ind: 'anc-visits', label: 'ANC visits (all)' },
      ],
    },
    { id: 's2', title: '2.2 eMTCT', rows: [{ id: 'r4', ind: 'anc-hiv-pos', label: 'Pregnant women newly HIV positive' }] },
    { id: 's3', title: 'Facility monitoring (not in the HMIS form)', rows: [{ id: 'r5', ind: 'anc-median-ga', label: 'Median gestational age at 1st visit (weeks)' }] },
  ],
  mappings: Object.fromEntries([
    ...ageMaps('AN01a', '2024-07-01').map(([col, m]) => [cellKey('r1', col), m] as const),
    [cellKey('r1', '10–14 years'), [{ de: 'AN01-2019', coc: 'y10', from: '2019-07-01', to: '2024-06-30' }, { de: 'AN01a', coc: 'y10', from: '2024-07-01', to: '' }]],
    ...ageMaps('AN02', '2024-07-01').map(([col, m]) => [cellKey('r3', col), m] as const),
    ...ageMaps('AN17', '2024-07-01')
      .slice(0, 2)
      .map(([col, m]) => [cellKey('r4', col), m] as const),
  ]),
}

// --- Columns and cells ------------------------------------------------------------------------------------

export type Col = { key: string; parts: string[]; tests: ((p: Patient) => boolean)[]; total: boolean }
/** The columns: bands of one Disaggregation, or every pair of two crossed, then an optional total. */
export function columns(r: Report, ind: Indicator, period: Period): Col[] {
  const [a, b] = r.columns
  const total: Col = { key: 'Total', parts: ['Total'], tests: [], total: true }
  if (!a) return [total]
  const first = bands(a, ind, period)
  const cols: Col[] = b
    ? first.flatMap((x) => bands(b, ind, period).map((y) => ({ key: `${x.label} × ${y.label}`, parts: [x.label, y.label], tests: [x.test, y.test], total: false })))
    : first.map((x) => ({ key: x.label, parts: [x.label], tests: [x.test], total: false }))
  return r.total ? [...cols, total] : cols
}
/** Column headers don't depend on the row's indicator except through the period date; use the first row's. */
export const headerColumns = (r: Report, period: Period) => columns(r, indicator(r.sections[0]?.rows[0]?.ind ?? 'anc-first') ?? (OTHERS[1] as Indicator), period)

export type Cell = { text: string; n: number | null; d: number | null; why?: string; patients: Patient[] }
export function cell(ind: Indicator, col: Col, period: Period, placeId: string): Cell {
  const place = PLACES.find((x) => x.id === placeId)
  if (place && place.level !== 'facility' && facilityOnly(ind)) return { text: 'n/a', n: null, d: null, why: `A ${ind.calc === 'median' ? 'median' : 'distinct count'} can't be rebuilt from facility summaries; it is shown at facility level only.`, patients: [] }
  const facilities = facilitiesUnder(placeId)
  const keep = (p: Patient) => facilities.includes(facilityOf(p)) && col.tests.every((t) => t(p))
  const r = evaluate(ind, period)
  const num = r.numerator.filter(keep)
  const den = r.denominator?.filter(keep) ?? null
  const dd = denOf(ind)
  if (dd !== 'one' && dd.population !== null) {
    if (!col.total) return { text: '–', n: null, d: null, why: 'Population Estimates are not split by band.', patients: num }
    const est = estimate(placeId, period)
    if (est === null) return { text: '–', n: null, d: null, why: `No population estimate for ${place?.label ?? placeId}: figures are given per district and summed upward, never split down to facilities.`, patients: num }
    const out = compute({ ...ind, den: { ...dd, population: Math.round(est * 10) / 10 } }, num, null, period)
    return { ...out, patients: num }
  }
  return { ...compute(ind, num, den, period), patients: num }
}

export const mappingFor = (list: Mapping[] | undefined, date: string) => list?.find((m) => m.from <= date && (m.to === '' || date <= m.to))

// --- Validation -------------------------------------------------------------------------------------------

export type Issue = { level: 'error' | 'warning' | 'info'; where: string; text: string; tab: 'basics' | 'rows' | 'columns' | 'dhis2' }
export function validate(r: Report): Issue[] {
  const issues: Issue[] = []
  const period = PERIOD_TYPES.find((p) => p.id === r.periodType)?.periods[0]
  if (!period) return issues
  if (r.levels.length === 0) issues.push({ level: 'error', where: 'Levels', text: 'Pick at least one level the Report runs for.', tab: 'basics' })
  if (r.columns.length > 2) issues.push({ level: 'error', where: 'Columns', text: 'At most two Disaggregations can be crossed.', tab: 'columns' })
  const above = r.levels.filter((l) => l !== 'facility')
  for (const s of r.sections)
    for (const row of s.rows) {
      const ind = indicator(row.ind)
      if (!ind) continue
      if (facilityOnly(ind) && above.length > 0) issues.push({ level: 'error', where: row.label, text: `Is a ${ind.calc === 'median' ? 'median' : 'distinct count'}, which can't be rebuilt from facility summaries because patient data never leaves the facility. Run this Report at facility level only, or move the row to a facility Report.`, tab: 'rows' })
      for (const d of r.columns) {
        const declared = ind.disaggregations.some((x) => JSON.stringify(x) === JSON.stringify(d))
        if (!declared) issues.push({ level: 'info', where: row.label, text: `The indicator doesn't list ${disaggLabel(d)}; this Report asks the edge to compute it by those bands too.`, tab: 'columns' })
      }
      if (!r.dhis2) continue
      if (!isCount(ind)) {
        const mapped = Object.keys(r.mappings).some((k) => k.startsWith(`${row.id}::`) && (r.mappings[k]?.length ?? 0) > 0)
        if (mapped) issues.push({ level: 'warning', where: row.label, text: `Is a ${ind.calc}, but DHIS2 data elements on this form hold counts. Send the counts it is built from instead.`, tab: 'dhis2' })
        continue
      }
      const today = '2026-11-17'
      const cols = columns(r, ind, period).filter((c) => !c.total && !c.parts.includes('not recorded'))
      const missing = cols.filter((c) => !mappingFor(r.mappings[cellKey(row.id, c.key)], today))
      if (missing.length > 0) issues.push({ level: 'error', where: row.label, text: `${missing.length} of ${cols.length} cells have no DHIS2 mapping in effect today (${missing.map((c) => c.key).join(', ')}).`, tab: 'dhis2' })
      for (const c of cols) {
        const list = r.mappings[cellKey(row.id, c.key)] ?? []
        for (const [i, m] of list.entries())
          for (const o of list.slice(i + 1)) if (m.from <= (o.to || '9999') && o.from <= (m.to || '9999')) issues.push({ level: 'error', where: `${row.label} · ${c.key}`, text: 'Two mappings are in effect over the same dates.', tab: 'dhis2' })
      }
    }
  return issues
}

export { AGE_SETS }
