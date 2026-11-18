// PROTOTYPE (#597), throwaway: made-up monthly summary MeasureReports for the dashboards prototype.
// Each facility has one summary per indicator and month (the parts needed to roll up: n and d, or a
// facility median). Higher levels are recomputed from summed parts (ADR 0070); medians stay at
// facility level; a facility that has not synced a month is missing, never zero (ADR 0075).

export type Level = 'facility' | 'district' | 'region' | 'national'
export type Place = { id: string; label: string; level: Level; parent: string | null; x: number; y: number }

// x/y: a schematic map position (0–100). Real boundaries ship in the national core package.
export const PLACES: Place[] = [
  { id: 'ug', label: 'Uganda', level: 'national', parent: null, x: 50, y: 50 },
  { id: 'central', label: 'Central region', level: 'region', parent: 'ug', x: 55, y: 65 },
  { id: 'northern', label: 'Northern region', level: 'region', parent: 'ug', x: 45, y: 25 },
  { id: 'kampala', label: 'Kampala', level: 'district', parent: 'central', x: 62, y: 68 },
  { id: 'wakiso', label: 'Wakiso', level: 'district', parent: 'central', x: 48, y: 70 },
  { id: 'mukono', label: 'Mukono', level: 'district', parent: 'central', x: 75, y: 62 },
  { id: 'gulu', label: 'Gulu', level: 'district', parent: 'northern', x: 40, y: 22 },
  { id: 'lira', label: 'Lira', level: 'district', parent: 'northern', x: 60, y: 30 },
  { id: 'kisenyi', label: 'Kisenyi HC IV', level: 'facility', parent: 'kampala', x: 60, y: 70 },
  { id: 'kawaala', label: 'Kawaala HC III', level: 'facility', parent: 'kampala', x: 64, y: 66 },
  { id: 'wakiso-hc4', label: 'Wakiso HC IV', level: 'facility', parent: 'wakiso', x: 46, y: 72 },
  { id: 'kasangati', label: 'Kasangati HC IV', level: 'facility', parent: 'wakiso', x: 50, y: 67 },
  { id: 'mukono-hc4', label: 'Mukono HC IV', level: 'facility', parent: 'mukono', x: 76, y: 63 },
  { id: 'gulu-rrh', label: 'Gulu RRH', level: 'facility', parent: 'gulu', x: 39, y: 21 },
  { id: 'awach', label: 'Awach HC IV', level: 'facility', parent: 'gulu', x: 43, y: 17 },
  { id: 'lira-rrh', label: 'Lira RRH', level: 'facility', parent: 'lira', x: 61, y: 31 },
]
export const placeOf = (id: string) => PLACES.find((p) => p.id === id)
export const childrenOf = (id: string) => PLACES.filter((p) => p.parent === id)
export const facilitiesUnder = (id: string): string[] => {
  const p = placeOf(id)
  if (!p) return []
  return p.level === 'facility' ? [p.id] : childrenOf(id).flatMap((c) => facilitiesUnder(c.id))
}
export const within = (root: string, id: string): boolean => {
  if (root === id) return true
  const p = placeOf(id)
  return p?.parent ? within(root, p.parent) : false
}
export const pathTo = (id: string): Place[] => {
  const p = placeOf(id)
  if (!p) return []
  return p.parent ? [...pathTo(p.parent), p] : [p]
}

// --- Indicators -------------------------------------------------------------------------------------------

export type Calc = 'count' | 'percent' | 'coverage' | 'median'
export type Band = { below: number; tone: 'bad' | 'warn' | 'good' }
export type Ind = { id: string; title: string; short: string; calc: Calc; unit: string; bands?: Band[] }
const UP: Band[] = [
  { below: 50, tone: 'bad' },
  { below: 80, tone: 'warn' },
  { below: Infinity, tone: 'good' },
]
export const INDICATORS: Ind[] = [
  { id: 'anc1-cov', title: 'ANC 1st visit coverage', short: 'ANC1 cov.', calc: 'coverage', unit: '% of expected pregnancies', bands: UP },
  { id: 'anc4-cov', title: 'ANC 4th visit coverage', short: 'ANC4 cov.', calc: 'coverage', unit: '% of expected pregnancies', bands: [{ below: 30, tone: 'bad' }, { below: 60, tone: 'warn' }, { below: Infinity, tone: 'good' }] },
  { id: 'ipt2', title: 'IPTp2 among ANC 1st visits', short: 'IPTp2', calc: 'percent', unit: '%', bands: UP },
  { id: 'hiv-anc', title: 'HIV tested at ANC', short: 'HIV tested', calc: 'percent', unit: '%', bands: [{ below: 80, tone: 'bad' }, { below: 95, tone: 'warn' }, { below: Infinity, tone: 'good' }] },
  { id: 'malaria-pos', title: 'Malaria test positivity', short: 'Malaria +', calc: 'percent', unit: '%', bands: [{ below: 20, tone: 'good' }, { below: 40, tone: 'warn' }, { below: Infinity, tone: 'bad' }] },
  { id: 'opd', title: 'OPD new attendances', short: 'OPD', calc: 'count', unit: 'visits' },
  { id: 'ga-anc1', title: 'Median gestational age at ANC 1st visit', short: 'GA at ANC1', calc: 'median', unit: 'weeks' },
]
export const indOf = (id: string) => INDICATORS.find((i) => i.id === id) as Ind

// --- Periods ----------------------------------------------------------------------------------------------

export const MONTHS = ['2025-07', '2025-08', '2025-09', '2025-10', '2025-11', '2025-12', '2026-01', '2026-02', '2026-03', '2026-04', '2026-05', '2026-06']
export const CURRENT = '2026-03' // today is in April 2026; March has just closed
export type PeriodType = 'month' | 'quarter' | 'fy'
export type Period = { id: string; label: string; months: string[] }
const MONTH_NAMES = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
const monthLabel = (m: string) => `${MONTH_NAMES[Number(m.slice(5)) - 1]} ${m.slice(2, 4)}`
const upTo = MONTHS.slice(0, MONTHS.indexOf(CURRENT) + 1)
export const PERIODS: Record<PeriodType, Period[]> = {
  month: upTo.map((m) => ({ id: m, label: monthLabel(m), months: [m] })),
  quarter: [
    { id: '2025Q3', label: 'Jul–Sep 25', months: MONTHS.slice(0, 3) },
    { id: '2025Q4', label: 'Oct–Dec 25', months: MONTHS.slice(3, 6) },
    { id: '2026Q1', label: 'Jan–Mar 26', months: MONTHS.slice(6, 9) },
  ],
  fy: [{ id: 'FY2025', label: 'FY 2025/26 to date', months: upTo }],
}
export const PERIOD_TYPE_LABEL: Record<PeriodType, string> = { month: 'Months', quarter: 'Quarters', fy: 'Financial year' }
export const range = (type: PeriodType, from: string, to: string): Period[] => {
  const list = PERIODS[type]
  const a = Math.max(0, list.findIndex((p) => p.id === from))
  const b = list.findIndex((p) => p.id === to)
  return list.slice(a, (b < 0 ? list.length - 1 : b) + 1)
}

// --- Summaries --------------------------------------------------------------------------------------------

type Summary = { n: number; d: number | null; median?: number }
const hash = (s: string) => {
  let h = 2166136261
  for (const c of s) h = Math.imul(h ^ c.charCodeAt(0), 16777619)
  return ((h >>> 0) % 1000) / 1000
}
const SIZE: Record<string, number> = { kisenyi: 1.6, kawaala: 0.7, 'wakiso-hc4': 1.2, kasangati: 0.9, 'mukono-hc4': 1, 'gulu-rrh': 1.8, awach: 0.6, 'lira-rrh': 1.4 }
// Expected pregnancies per year (Population package, ADR 0071), per facility catchment for the prototype.
const EXPECTED: Record<string, number> = { kisenyi: 1500, kawaala: 700, 'wakiso-hc4': 1300, kasangati: 900, 'mukono-hc4': 1000, 'gulu-rrh': 1700, awach: 650, 'lira-rrh': 1400 }
// Last summary each facility synced. Kawaala's edge has been offline since mid-February; Awach missed December.
export const SYNCED: Record<string, string> = { kisenyi: '2026-04-02 06:10', kawaala: '2026-02-14 17:42', 'wakiso-hc4': '2026-04-02 05:55', kasangati: '2026-04-01 22:03', 'mukono-hc4': '2026-04-02 06:01', 'gulu-rrh': '2026-04-02 04:48', awach: '2026-04-01 19:30', 'lira-rrh': '2026-04-02 06:12' }
const missing = (f: string, m: string) => (f === 'kawaala' && m >= '2026-02') || (f === 'awach' && m === '2025-12')

export function summary(f: string, ind: Ind, m: string): Summary | null {
  if (missing(f, m) || m > CURRENT) return null
  const r = hash(`${f}|${ind.id}|${m}`)
  const size = SIZE[f] ?? 1
  const season = 1 + 0.35 * Math.sin(((MONTHS.indexOf(m) + 2) / 12) * 2 * Math.PI)
  const north = f === 'gulu-rrh' || f === 'awach' || f === 'lira-rrh'
  switch (ind.id) {
    case 'anc1-cov': {
      const exp = (EXPECTED[f] ?? 1000) / 12
      return { n: Math.round(exp * (0.7 + 0.35 * r)), d: exp }
    }
    case 'anc4-cov': {
      const exp = (EXPECTED[f] ?? 1000) / 12
      return { n: Math.round(exp * ((north ? 0.3 : 0.45) + 0.3 * r)), d: exp }
    }
    case 'ipt2': {
      const d = Math.round(100 * size * (0.8 + 0.4 * r))
      return { n: Math.round(d * (0.55 + 0.4 * hash(`${m}${f}ipt`))), d }
    }
    case 'hiv-anc': {
      const d = Math.round(100 * size * (0.8 + 0.4 * r))
      return { n: Math.round(d * (0.85 + 0.14 * hash(`${m}${f}hiv`))), d }
    }
    case 'malaria-pos': {
      const d = Math.round(600 * size * season)
      return { n: Math.round(d * ((north ? 0.32 : 0.16) * season + 0.1 * r)), d }
    }
    case 'opd':
      return { n: Math.round(1800 * size * season * (0.9 + 0.2 * r)), d: null }
    default:
      return { n: 0, d: null, median: Math.round(16 + 8 * r) }
  }
}

export type Value = { v: number | null; n: number; d: number | null; reported: number; expected: number; missing: string[]; why?: string }
const fmt0 = (x: number) => Math.round(x).toLocaleString('en-GB')
export function value(ind: Ind, placeId: string, months: string[]): Value {
  const facs = facilitiesUnder(placeId)
  const place = placeOf(placeId)
  if (ind.calc === 'median' && place?.level !== 'facility')
    return { v: null, n: 0, d: null, reported: 0, expected: facs.length, missing: [], why: 'A median cannot be rebuilt from facility summaries; it shows at facility level only.' }
  let n = 0
  let d = 0
  let reported = 0
  const gaps: string[] = []
  const medians: number[] = []
  for (const f of facs) {
    const rows = months.map((m) => summary(f, ind, m))
    if (rows.every(Boolean)) reported += 1
    else gaps.push(placeOf(f)?.label ?? f)
    for (const s of rows) {
      if (!s) continue
      n += s.n
      d += s.d ?? 0
      if (s.median !== undefined) medians.push(s.median)
    }
  }
  if (reported === 0 && gaps.length > 0 && n === 0) return { v: null, n, d, reported, expected: facs.length, missing: gaps }
  const v = ind.calc === 'count' ? n : ind.calc === 'median' ? ([...medians].sort((a, b) => a - b)[Math.floor(medians.length / 2)] ?? null) : d > 0 ? (100 * n) / d : null
  return { v, n, d: ind.calc === 'count' || ind.calc === 'median' ? null : d, reported, expected: facs.length, missing: gaps }
}
export const show = (ind: Ind, v: number | null) => (v === null ? '—' : ind.calc === 'count' ? fmt0(v) : ind.calc === 'median' ? `${v} wk` : `${v.toFixed(1)}%`)
export const toneOf = (ind: Ind, v: number | null) => (v === null || !ind.bands ? null : (ind.bands.find((b) => v < b.below)?.tone ?? null))

// --- Disaggregation (made-up split of the same summaries) -----------------------------------------------------

export const DISAGGS: { id: string; label: string; bands: { label: string; share: number }[] }[] = [
  { id: 'none', label: 'None', bands: [] },
  { id: 'age-anc', label: 'Age (ANC bands)', bands: [ { label: '10–19', share: 0.22 }, { label: '20–24', share: 0.31 }, { label: '25–49', share: 0.47 } ] },
  { id: 'sex', label: 'Sex', bands: [ { label: 'Female', share: 0.57 }, { label: 'Male', share: 0.43 } ] },
]

// --- Visualizations and Dashboards ----------------------------------------------------------------------------

export type ChartType = 'column' | 'line' | 'table' | 'single' | 'map'
export type Vis = {
  id: string
  title: string
  type: ChartType
  inds: string[]
  places: string[] // explicit places; 'children' expands to the children of the scope root
  children: boolean
  periodType: PeriodType
  from: string
  to: string
  series: 'ind' | 'place' | 'disagg'
  disagg: string
}
export type Share = { kind: 'person' | 'role' | 'level'; label: string }
export type Dashboard = { id: string; title: string; kind: 'standard' | 'saved'; source: string; shared: Share[]; items: string[] }
export type Role = { id: string; label: string; root: string; node: 'central' | 'edge'; author: boolean }
export const ROLES: Role[] = [
  { id: 'national', label: 'National M&E officer', root: 'ug', node: 'central', author: true },
  { id: 'dho', label: 'Kampala district biostatistician', root: 'kampala', node: 'central', author: true },
  { id: 'facility', label: 'Kisenyi HC IV in-charge', root: 'kisenyi', node: 'edge', author: false },
]

export const INITIAL_VIS: Vis[] = [
  { id: 'v1', title: 'ANC coverage by month', type: 'line', inds: ['anc1-cov', 'anc4-cov'], places: [], children: false, periodType: 'month', from: '2025-07', to: CURRENT, series: 'ind', disagg: 'none' },
  { id: 'v2', title: 'ANC 4th visit coverage by area', type: 'column', inds: ['anc4-cov'], places: [], children: true, periodType: 'quarter', from: '2025Q3', to: '2026Q1', series: 'place', disagg: 'none' },
  { id: 'v3', title: 'HIV tested at ANC, this quarter', type: 'single', inds: ['hiv-anc'], places: [], children: false, periodType: 'quarter', from: '2026Q1', to: '2026Q1', series: 'ind', disagg: 'none' },
  { id: 'v4', title: 'Malaria positivity map', type: 'map', inds: ['malaria-pos'], places: [], children: true, periodType: 'month', from: CURRENT, to: CURRENT, series: 'place', disagg: 'none' },
  { id: 'v5', title: 'ANC indicators by area', type: 'table', inds: ['anc1-cov', 'anc4-cov', 'ipt2', 'hiv-anc', 'ga-anc1'], places: [], children: true, periodType: 'fy', from: 'FY2025', to: 'FY2025', series: 'place', disagg: 'none' },
  { id: 'v6', title: 'OPD attendances by month', type: 'column', inds: ['opd'], places: [], children: false, periodType: 'month', from: '2025-07', to: CURRENT, series: 'ind', disagg: 'none' },
]
export const INITIAL_DASHBOARDS: Dashboard[] = [
  { id: 'd1', title: 'ANC overview', kind: 'standard', source: 'Reporting package ug.moh.reporting 2026.1', shared: [{ kind: 'level', label: 'Everyone, any level' }], items: ['v3', 'v1', 'v2', 'v5'] },
  { id: 'd2', title: 'Malaria and OPD watch', kind: 'saved', source: 'Saved by Kampala district biostatistician', shared: [{ kind: 'role', label: 'District health teams' }, { kind: 'person', label: 'Dr. A. Nakato (DHO)' }], items: ['v4', 'v6'] },
]
