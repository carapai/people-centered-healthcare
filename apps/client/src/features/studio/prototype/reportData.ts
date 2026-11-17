// PROTOTYPE (#544), throwaway: a Report (ADR 0070) over the indicator prototype's test patients.
// Round 2 (layout B chosen): a Report is sections, and each section is either
// - a table: indicator rows sharing the section's Disaggregations, each Disaggregation laid out as
//   rows or as columns, with optional column totals, row totals and % change from the previous period; or
// - a line list: one row per enrollment or per visit, with any attribute or any concept as a
//   column (facility level only).
// A Report sent to DHIS2 maps each cell of the sections it sends. Places, facilities and
// population figures are made up.
import {
  AGE_SETS,
  INITIAL,
  INPUTS,
  PATIENTS,
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

const cond = (input: string, value: string, op: '=' | '>=' = '='): Node => ({ kind: 'cond', input, op, value })
const firstVisit = (...more: Node[]): Node => ({ kind: 'group', join: 'all', children: [cond('sex', 'female'), cond('type', 'anc-first'), ...more] })
const base = OTHERS[1] as Indicator
const ageAnc: Disagg = { kind: 'age', set: 'moh-age-anc' }
const hiv: Disagg = { kind: 'coded', input: 'hiv' }

export const CATALOGUE: Indicator[] = [
  { ...(OTHERS[0] as Indicator), disaggregations: [ageAnc] },
  { ...base, disaggregations: [ageAnc, hiv] },
  { ...INITIAL, version: '2.4.0' },
  { ...base, id: 'anc-first-late', title: 'ANC 1st visit at 12 weeks or later', version: '1.0.0', numerator: { tree: firstVisit(cond('ga', '12', '>=')) }, disaggregations: [ageAnc] },
  { ...base, id: 'anc-hiv-pos', title: 'Pregnant women HIV positive at ANC 1st visit', version: '1.1.0', numerator: { tree: firstVisit(cond('hiv', 'positive')) }, disaggregations: [ageAnc] },
  { ...base, id: 'anc-median-ga', title: 'Median gestational age at ANC 1st visit', version: '1.0.0', calc: 'median', value: { kind: 'input', input: 'ga' }, pick: 'first', numerator: { tree: firstVisit() }, disaggregations: [] },
  { ...base, id: 'anc-districts', title: 'Districts ANC clients come from', version: '1.0.0', calc: 'distinct', distinctOf: 'district', numerator: { tree: firstVisit() }, disaggregations: [] },
  { ...base, id: 'anc-coverage', title: 'ANC 1st visit coverage', version: '1.2.0', calc: 'percentage', den: { tree: { kind: 'group', join: 'all', children: [] }, population: 0 }, numerator: { tree: firstVisit() }, disaggregations: [] },
]
const PACKAGE_OF: Record<string, string> = { 'anc-hiv-pos': 'ug.moh.emtct' }
export const indicator = (id: string) => CATALOGUE.find((i) => i.id === id)
export const canonical = (i: Indicator) => `https://health.go.ug/fhir/${PACKAGE_OF[i.id] ?? 'ug.moh.anc'}/Measure/${i.id}|${i.version}`
/** Medians and distinct counts can't be rebuilt from facility summaries (ADR 0070). */
export const facilityOnly = (i: Indicator) => i.calc === 'median' || i.calc === 'distinct'
/** DHIS2 aggregate data elements hold counts. */
export const isCount = (i: Indicator) => i.calc === 'count' || i.calc === 'sum'
export const DISAGGS: Disagg[] = [...AGE_SETS.map((a): Disagg => ({ kind: 'age', set: a.id })), ...INPUTS.filter((i) => i.type !== 'number').map((c): Disagg => ({ kind: 'coded', input: c.id }))]
export const encD = (d: Disagg) => (d.kind === 'age' ? `age:${d.set}` : `coded:${d.input}`)
export const decD = (s: string): Disagg => {
  const [k = '', v = ''] = s.split(':')
  return k === 'age' ? { kind: 'age', set: v } : { kind: 'coded', input: v }
}
export { disaggLabel }

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
export const placeOf = (id: string) => PLACES.find((x) => x.id === id)
export const facilitiesUnder = (id: string): string[] => {
  const place = placeOf(id)
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

// --- Periods --------------------------------------------------------------------------------------------

const MONTHS: Record<PeriodType, number> = { weekly: 0, monthly: 1, quarterly: 3, semiAnnual: 6, annual: 12, financialJuly: 12 }
const iso = (d: Date) => d.toISOString().slice(0, 10)
const addDays = (date: string, n: number) => {
  const d = new Date(`${date}T00:00:00Z`)
  d.setUTCDate(d.getUTCDate() + n)
  return iso(d)
}
const addMonths = (date: string, n: number) => {
  const d = new Date(`${date}T00:00:00Z`)
  d.setUTCMonth(d.getUTCMonth() + n)
  return iso(d)
}
/** The period before this one, of the same type. */
export function previous(p: Period): Period {
  const list = PERIOD_TYPES.find((t) => t.id === p.type)?.periods ?? []
  const i = list.findIndex((x) => x.label === p.label)
  const known = i > 0 ? list[i - 1] : undefined
  if (known) return known
  const start = p.type === 'weekly' ? addDays(p.start, -7) : addMonths(p.start, -MONTHS[p.type])
  return { type: p.type, label: `before ${p.label}`, start, end: addDays(p.start, -1) }
}

// --- DHIS2 metadata (made up; the real list comes from the instance at Validation) -------------------------

export const DATA_SETS = [{ id: 'hmis105m', label: 'HMIS 105: Health unit outpatient monthly report (2024)' }]
export const DATA_ELEMENTS = [
  { id: 'AN01a', label: '105-AN01a. ANC 1st visit for women' },
  { id: 'AN01c', label: '105-AN01c. ANC 1st visit at 12 weeks or later' },
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

export type Axis = 'rows' | 'columns'
export type Row = { id: string; ind: string; label: string }
// --- Line lists -----------------------------------------------------------------------------------------

/** One row per enrollment in a program, or one row per visit. */
export type ListBase = 'enrollments' | 'visits'
/** For an enrollment row, which of its visits' values a concept column shows. */
export type Pick = 'latest' | 'first' | 'all' | 'count'
export type ListColumn = { id: string; src: { kind: 'attr'; id: string } | { kind: 'concept'; id: string; pick: Pick }; label: string }
export type ListSection = {
  kind: 'list'
  id: string
  title: string
  base: ListBase
  program: string
  visitTypes: string[]
  when: 'inPeriod' | 'active'
  ind: string | null
  columns: ListColumn[]
}
export const PROGRAMS = [
  { id: 'anc', label: 'Antenatal care (ug.moh.anc)', visitTypes: ['anc-first', 'anc-followup'] },
  { id: 'any', label: 'Any program (all visits)', visitTypes: ['anc-first', 'anc-followup', 'opd'] },
]
const inProgram = (program: string, p: Patient) => (PROGRAMS.find((x) => x.id === program)?.visitTypes ?? []).includes(p.encounterType)

/** A list row: one enrollment (all its visits) or one visit. */
export type ListRow = { key: string; person: Patient; visits: Patient[] }
type Attr = { id: string; label: string; group: 'Person' | 'Enrollment' | 'Visit'; get: (r: ListRow) => string; base?: ListBase }
const first = (r: ListRow) => r.visits[0] ?? r.person
const last = (r: ListRow) => r.visits.at(-1) ?? r.person
/** Made-up but stable values for concepts the test patients don't carry. */
const hash = (s: string) => [...s].reduce((a, c) => (a * 31 + c.charCodeAt(0)) % 9973, 7)
const fake = (p: Patient, salt: string, lo: number, hi: number, dp = 0, key = p.id) => (lo + ((hash(key + salt) % 1000) / 1000) * (hi - lo)).toFixed(dp)
const ageNow = (dob: string) => String(Math.floor((Date.parse('2026-11-18') - Date.parse(dob)) / (365.25 * 864e5)))

export const ATTRIBUTES: Attr[] = [
  { id: 'name', label: 'Name', group: 'Person', get: (r) => r.person.name },
  { id: 'sex', label: 'Sex', group: 'Person', get: (r) => r.person.sex },
  { id: 'dob', label: 'Date of birth', group: 'Person', get: (r) => r.person.dob },
  { id: 'age', label: 'Age (years)', group: 'Person', get: (r) => ageNow(r.person.dob) },
  { id: 'phone', label: 'Phone', group: 'Person', get: (r) => `07${fake(r.person, 'tel', 1e7, 9.9e7, 0, r.person.person)}` },
  { id: 'district', label: 'District of residence', group: 'Person', get: (r) => r.person.district || 'not recorded' },
  { id: 'nin', label: 'National ID (NIN)', group: 'Person', get: (r) => (hash(r.person.person) % 3 === 0 ? '' : `CF${fake(r.person, 'nin', 1e11, 9e11, 0, r.person.person)}`) },
  { id: 'enrolled', label: 'Enrollment date', group: 'Enrollment', get: (r) => first(r).dates.enrolled ?? '' },
  { id: 'status', label: 'Enrollment status', group: 'Enrollment', get: (r) => (last(r).dates.eocEnd ? 'completed' : 'active') },
  { id: 'visits', label: 'Visits in the enrollment', group: 'Enrollment', get: (r) => String(r.visits.length), base: 'enrollments' },
  { id: 'lastVisit', label: 'Last visit date', group: 'Enrollment', get: (r) => last(r).dates.start ?? '', base: 'enrollments' },
  { id: 'start', label: 'Visit date', group: 'Visit', get: (r) => first(r).dates.start ?? '', base: 'visits' },
  { id: 'type', label: 'Visit type', group: 'Visit', get: (r) => first(r).encounterType, base: 'visits' },
  { id: 'facility', label: 'Facility', group: 'Visit', get: (r) => placeOf(facilityOf(first(r)))?.label ?? '' },
  { id: 'recorded', label: 'Entered on', group: 'Visit', get: (r) => first(r).dates.recorded ?? '', base: 'visits' },
]

type Concept = { id: string; code: string; label: string; unit?: string; get: (p: Patient) => string }
/** Concepts from the terminology server (a few of thousands); any can be a column. */
export const CONCEPTS: Concept[] = [
  { id: 'ga', code: 'ug-anc#gestational-age', label: 'Gestational age', unit: 'weeks', get: (p) => (p.encounterType.startsWith('anc') ? String(p.gestationWeeks) : '') },
  { id: 'hiv', code: 'ug-core#hiv-status', label: 'HIV status', get: (p) => p.hiv },
  { id: 'lmp', code: 'ug-anc#lmp', label: 'Last menstrual period', get: (p) => p.dates.lmp ?? '' },
  { id: 'edd', code: 'ug-anc#edd', label: 'Expected date of delivery', get: (p) => (p.dates.lmp ? new Date(Date.parse(p.dates.lmp) + 280 * 864e5).toISOString().slice(0, 10) : '') },
  { id: 'gravida', code: 'ug-anc#gravida', label: 'Gravida', get: (p) => (p.firstPregnancy ? '1' : fake(p, 'g', 2, 5)) },
  { id: 'hb', code: 'ug-lab#haemoglobin', label: 'Haemoglobin', unit: 'g/dL', get: (p) => fake(p, 'hb', 8.5, 13.5, 1) },
  { id: 'weight', code: 'ug-core#body-weight', label: 'Weight', unit: 'kg', get: (p) => fake(p, 'w', 48, 85) },
  { id: 'sbp', code: 'ug-core#systolic-bp', label: 'Systolic blood pressure', unit: 'mmHg', get: (p) => fake(p, 'sbp', 100, 150) },
  { id: 'dbp', code: 'ug-core#diastolic-bp', label: 'Diastolic blood pressure', unit: 'mmHg', get: (p) => fake(p, 'dbp', 60, 95) },
  { id: 'muac', code: 'ug-core#muac', label: 'MUAC', unit: 'cm', get: (p) => fake(p, 'muac', 21, 30, 1) },
  { id: 'syphilis', code: 'ug-lab#syphilis-rdt', label: 'Syphilis test result', get: (p) => (hash(p.id) % 7 === 0 ? 'reactive' : 'non-reactive') },
  { id: 'ipt', code: 'ug-anc#iptp-dose', label: 'IPTp-SP dose given', get: (p) => (p.encounterType === 'anc-followup' ? fake(p, 'ipt', 1, 3) : '') },
  { id: 'llin', code: 'ug-anc#llin-given', label: 'Mosquito net given', get: (p) => (p.encounterType === 'anc-first' ? 'yes' : '') },
  { id: 'next', code: 'ug-core#next-appointment', label: 'Next appointment date', get: (p) => p.dates.nextVisit ?? '' },
]
export const PICKS: { id: Pick; label: string }[] = [
  { id: 'latest', label: 'latest value' },
  { id: 'first', label: 'first value' },
  { id: 'all', label: 'all values, with dates' },
  { id: 'count', label: 'times recorded' },
]
export const attribute = (id: string) => ATTRIBUTES.find((a) => a.id === id)
export const concept = (id: string) => CONCEPTS.find((c) => c.id === id)

const byDate = (list: Patient[]) => {
  const out: Patient[] = []
  for (const p of list) {
    const at = out.findIndex((q) => (q.dates.start ?? '') > (p.dates.start ?? ''))
    out.splice(at === -1 ? out.length : at, 0, p)
  }
  return out
}

/** The rows of a line list for a period and place, oldest first. */
export function listRows(s: ListSection, period: Period, placeId: string): ListRow[] {
  const facilities = facilitiesUnder(placeId)
  const counted = s.ind ? new Set(evaluate(indicator(s.ind) as Indicator, period).numerator.map((p) => (s.base === 'visits' ? p.id : p.enrollment))) : null
  const all = PATIENTS.filter((p) => facilities.includes(facilityOf(p)) && inProgram(s.program, p))
  const inPeriod = (d?: string) => Boolean(d) && (d ?? '') >= period.start && (d ?? '') <= period.end
  if (s.base === 'visits')
    return byDate(all.filter((p) => inPeriod(p.dates.start) && (s.visitTypes.length === 0 || s.visitTypes.includes(p.encounterType)) && (!counted || counted.has(p.id)))).map((p) => ({ key: p.id, person: p, visits: [p] }))
  const enrollments = [...new Set(all.map((p) => p.enrollment))]
  return enrollments
    .map((e) => ({ key: e, visits: byDate(all.filter((p) => p.enrollment === e && (p.dates.start ?? '') <= period.end)) }))
    .filter(({ key, visits }) => {
      const v = visits[0]
      if (!v || (counted && !counted.has(key))) return false
      const enrolled = v.dates.enrolled ?? ''
      const ended = visits.at(-1)?.dates.eocEnd
      return s.when === 'inPeriod' ? inPeriod(enrolled) : enrolled <= period.end && (!ended || ended >= period.start)
    })
    .map(({ key, visits }) => ({ key, person: visits[0] as Patient, visits }))
}

export function listValue(c: ListColumn, r: ListRow): string {
  if (c.src.kind === 'attr') return attribute(c.src.id)?.get(r) ?? ''
  const con = concept(c.src.id)
  if (!con) return ''
  const values = r.visits.map((v) => ({ at: v.dates.start ?? '', v: con.get(v) })).filter((x) => x.v !== '')
  const unit = con.unit ? ` ${con.unit}` : ''
  if (c.src.pick === 'count') return String(values.length)
  if (values.length === 0) return ''
  if (c.src.pick === 'all') return values.map((x) => `${x.v}${unit} (${x.at})`).join('; ')
  const x = c.src.pick === 'first' ? values[0] : values.at(-1)
  return x ? `${x.v}${unit}` : ''
}

export type TableSection = {
  kind: 'table'
  id: string
  title: string
  disaggs: { d: Disagg; axis: Axis }[]
  rows: Row[]
  colTotals: boolean
  rowTotals: boolean
  change: boolean
  send: boolean
}
export type Section = TableSection | ListSection
export type Report = {
  title: string
  pkg: string
  version: string
  periodType: PeriodType
  levels: Level[]
  sections: Section[]
  dhis2: string | null
  mappings: Record<string, Mapping[]>
}
export const cellKey = (row: string, rk: string, ck: string) => `${row}::${rk}::${ck}`

const AGE_COCS: [string, string][] = [
  ['10–14 years', 'y10'],
  ['15–19 years', 'y15'],
  ['20–24 years', 'y20'],
  ['25+ years', 'y25'],
]
const ageMaps = (row: string, de: string, skip: string[] = []) => AGE_COCS.filter(([col]) => !skip.includes(col)).map(([col, coc]): [string, Mapping[]] => [cellKey(row, '', col), [{ de, coc, from: '2024-07-01', to: '' }]])

export const INITIAL_REPORT: Report = {
  title: 'HMIS 105 · 2 Antenatal care',
  pkg: 'ug.moh.hmis',
  version: '4.2.0-draft',
  periodType: 'monthly',
  levels: ['facility', 'district', 'national'],
  dhis2: 'hmis105m',
  sections: [
    {
      kind: 'table',
      id: 's1',
      title: '2.1 ANC attendance',
      disaggs: [{ d: ageAnc, axis: 'columns' }],
      rows: [
        { id: 'r1', ind: 'anc-first', label: 'ANC 1st visit for women' },
        { id: 'r2', ind: 'anc-first-late', label: 'ANC 1st visit at 12 weeks or later' },
        { id: 'r3', ind: 'anc-visits', label: 'ANC visits (all)' },
      ],
      colTotals: false,
      rowTotals: true,
      change: true,
      send: true,
    },
    {
      kind: 'table',
      id: 's2',
      title: '2.2 ANC 1st visits by HIV status',
      disaggs: [
        { d: hiv, axis: 'rows' },
        { d: ageAnc, axis: 'columns' },
      ],
      rows: [{ id: 'r4', ind: 'anc-first', label: 'ANC 1st visit for women' }],
      colTotals: true,
      rowTotals: true,
      change: false,
      send: false,
    },
    {
      kind: 'table',
      id: 's3',
      title: 'Facility monitoring (not in the HMIS form)',
      disaggs: [],
      rows: [
        { id: 'r5', ind: 'anc-first-12w', label: 'ANC 1st visit within 12 weeks (%)' },
        { id: 'r6', ind: 'anc-median-ga', label: 'Median gestational age at 1st visit (weeks)' },
      ],
      colTotals: false,
      rowTotals: false,
      change: true,
      send: false,
    },
    {
      kind: 'list',
      id: 's4',
      title: 'Women in ANC this period',
      base: 'enrollments',
      program: 'anc',
      visitTypes: [],
      when: 'active',
      ind: null,
      columns: [
        { id: 'c1', src: { kind: 'attr', id: 'name' }, label: 'Name' },
        { id: 'c2', src: { kind: 'attr', id: 'phone' }, label: 'Phone' },
        { id: 'c3', src: { kind: 'attr', id: 'enrolled' }, label: 'Enrolled' },
        { id: 'c4', src: { kind: 'attr', id: 'visits' }, label: 'Visits' },
        { id: 'c5', src: { kind: 'concept', id: 'ga', pick: 'latest' }, label: 'Gestational age (latest)' },
        { id: 'c6', src: { kind: 'concept', id: 'hb', pick: 'all' }, label: 'Haemoglobin (all)' },
        { id: 'c7', src: { kind: 'concept', id: 'hiv', pick: 'latest' }, label: 'HIV status' },
        { id: 'c8', src: { kind: 'concept', id: 'next', pick: 'latest' }, label: 'Next appointment' },
      ],
    },
  ],
  mappings: Object.fromEntries([
    ...ageMaps('r1', 'AN01a'),
    [cellKey('r1', '', '10–14 years'), [{ de: 'AN01-2019', coc: 'y10', from: '2019-07-01', to: '2024-06-30' }, { de: 'AN01a', coc: 'y10', from: '2024-07-01', to: '' }]],
    ...ageMaps('r2', 'AN01c'),
    ...ageMaps('r3', 'AN02', ['25+ years']),
  ]),
}

// --- Bands and cells --------------------------------------------------------------------------------------

export type Test = (p: Patient) => boolean
export type Combo = { key: string; parts: string[]; tests: Test[] }
/** Every combination of the bands of these Disaggregations, first one outermost. */
export function cross(ds: Disagg[], ind: Indicator, period: Period): Combo[] {
  let out: Combo[] = [{ key: '', parts: [], tests: [] }]
  for (const d of ds) {
    const bs = bands(d, ind, period)
    out = out.flatMap((c) => bs.map((b) => ({ key: [...c.parts, b.label].join(' × '), parts: [...c.parts, b.label], tests: [...c.tests, b.test] })))
  }
  return out
}
export const axisOf = (s: TableSection, axis: Axis) => s.disaggs.filter((x) => x.axis === axis).map((x) => x.d)

export type Cell = { text: string; value: number | null; n: number | null; d: number | null; why?: string; patients: Patient[] }
export function cell(ind: Indicator, tests: Test[], period: Period, placeId: string): Cell {
  const place = placeOf(placeId)
  if (place && place.level !== 'facility' && facilityOnly(ind)) return { text: 'n/a', value: null, n: null, d: null, why: `A ${ind.calc === 'median' ? 'median' : 'distinct count'} can't be rebuilt from facility summaries; it is shown at facility level only.`, patients: [] }
  const facilities = facilitiesUnder(placeId)
  const keep = (p: Patient) => facilities.includes(facilityOf(p)) && tests.every((t) => t(p))
  const r = evaluate(ind, period)
  const num = r.numerator.filter(keep)
  const den = r.denominator?.filter(keep) ?? null
  const dd = denOf(ind)
  let out: { text: string; n: number; d: number | null }
  if (dd !== 'one' && dd.population !== null) {
    if (tests.length > 0) return { text: '–', value: null, n: null, d: null, why: 'Population Estimates are not split by band.', patients: num }
    const est = estimate(placeId, period)
    if (est === null) return { text: '–', value: null, n: null, d: null, why: `No population estimate for ${place?.label ?? placeId}: figures are given per district and summed upward, never split down to facilities.`, patients: num }
    out = compute({ ...ind, den: { ...dd, population: Math.round(est * 10) / 10 } }, num, null, period)
  } else out = compute(ind, num, den, period)
  const value = Number.parseFloat(out.text)
  return { ...out, value: Number.isNaN(value) ? null : value, patients: num }
}
/** % change from the previous period, as text. */
export function change(now: Cell, before: Cell): string {
  if (now.value === null || before.value === null) return ''
  if (before.value === 0) return now.value === 0 ? '' : 'new'
  const pct = Math.round((100 * (now.value - before.value)) / before.value)
  return `${pct > 0 ? '▲' : pct < 0 ? '▼' : ''}${Math.abs(pct)}%`
}

export const mappingFor = (list: Mapping[] | undefined, date: string) => list?.find((m) => m.from <= date && (m.to === '' || date <= m.to))

/** The cells of a table section that DHIS2 needs mapped: count rows, real bands (no totals, no "not recorded"). */
export function sentCells(s: TableSection, period: Period) {
  return s.rows.flatMap((rw) => {
    const ind = indicator(rw.ind)
    if (!ind) return []
    const rows = cross(axisOf(s, 'rows'), ind, period)
    const cols = cross(axisOf(s, 'columns'), ind, period)
    return rows.flatMap((rc) => cols.map((cc) => ({ rw, ind, rk: rc.key, ck: cc.key, needed: isCount(ind) && ![...rc.parts, ...cc.parts].includes('not recorded') })))
  })
}

// --- Validation -------------------------------------------------------------------------------------------

export type Issue = { level: 'error' | 'warning' | 'info'; where: string; text: string }
export function validate(r: Report): Issue[] {
  const issues: Issue[] = []
  const period = PERIOD_TYPES.find((p) => p.id === r.periodType)?.periods[0]
  if (!period) return issues
  if (r.levels.length === 0) issues.push({ level: 'error', where: 'Levels', text: 'Pick at least one level the Report runs for.' })
  const above = r.levels.some((l) => l !== 'facility')
  for (const s of r.sections) {
    if (s.kind === 'list') {
      if (above) issues.push({ level: 'error', where: s.title, text: 'A line list shows patient-level records, which never leave the facility. Run this Report at facility level only, or move the line list to a facility Report.' })
      continue
    }
    for (const row of s.rows) {
      const ind = indicator(row.ind)
      if (!ind) continue
      if (facilityOnly(ind) && above) issues.push({ level: 'error', where: row.label, text: `Is a ${ind.calc === 'median' ? 'median' : 'distinct count'}, which can't be rebuilt from facility summaries because patient data never leaves the facility. Run this Report at facility level only, or move the row to a facility Report.` })
      const missingD = s.disaggs.filter((x) => !ind.disaggregations.some((y) => encD(y) === encD(x.d)))
      if (missingD.length > 0) issues.push({ level: 'info', where: `${s.title} · ${row.label}`, text: `The indicator doesn't list ${missingD.map((x) => disaggLabel(x.d)).join(' or ')}; this Report asks the edge to compute it by those bands too.` })
    }
    if (!r.dhis2 || !s.send) continue
    const cells = sentCells(s, period)
    for (const rw of s.rows) {
      const ind = indicator(rw.ind)
      if (ind && !isCount(ind)) issues.push({ level: 'warning', where: `${s.title} · ${rw.label}`, text: `Is a ${ind.calc}; DHIS2 data elements on this form hold counts, so this row isn't sent. Send the counts it is built from instead.` })
    }
    const missing = cells.filter((c) => c.needed && !mappingFor(r.mappings[cellKey(c.rw.id, c.rk, c.ck)], '2026-11-18'))
    if (missing.length > 0) issues.push({ level: 'error', where: s.title, text: `${missing.length} cells have no DHIS2 mapping in effect today: ${missing.map((c) => `${c.rw.label} · ${[c.rk, c.ck].filter(Boolean).join(' · ') || 'value'}`).join('; ')}.` })
    for (const c of cells) {
      const list = r.mappings[cellKey(c.rw.id, c.rk, c.ck)] ?? []
      for (const [i, m] of list.entries())
        for (const o of list.slice(i + 1)) if (m.from <= (o.to || '9999') && o.from <= (m.to || '9999')) issues.push({ level: 'error', where: `${c.rw.label} · ${c.ck}`, text: 'Two mappings are in effect over the same dates.' })
    }
  }
  return issues
}
