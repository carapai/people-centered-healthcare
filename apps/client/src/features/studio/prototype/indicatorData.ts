// PROTOTYPE (#534), throwaway: one ANC indicator as a FHIR R5 Measure would hold it (populations
// with condition trees compiled to FHIRPath, a period date, disaggregations), sample test
// patients, and a tiny evaluator. The real evaluation is the server's FHIRPath engine (ADR 0068).

export type Input = { id: string; label: string; path: string; type: 'code' | 'number' | 'boolean'; values?: string[] }
export type Op = '=' | '!=' | '<' | '<=' | '>' | '>='
export type Join = 'all' | 'any' | 'not'
export type Node = { kind: 'cond'; input: string; op: Op; value: string } | { kind: 'group'; join: Join; children: Node[] }
export type PopKey = 'initial' | 'denominator' | 'exclusion' | 'numerator'
export type Population = { key: PopKey; label: string; tree: Node }
export type Kind = 'count' | 'proportion'
export type PeriodDate = { id: 'start' | 'recorded' | 'lmp'; label: string; path: string }
export type Disaggregation = { id: string; label: string; shared: boolean; bands: { label: string; test: (p: Patient) => boolean }[] }
export type Patient = { id: string; name: string; age: number; sex: 'female' | 'male'; encounterType: string; gestationWeeks: number; firstPregnancy: boolean; start: string; recorded: string; lmp: string }
export type Indicator = { title: string; url: string; kind: Kind; periodDate: PeriodDate['id']; populations: Population[]; disaggregations: string[] }
export type SavedTest = { period: string; expect: Record<string, number> }

export const INPUTS: Input[] = [
  { id: 'type', label: 'Visit type', path: '%encounter.type.coding.code', type: 'code', values: ['anc-first', 'anc-followup', 'opd'] },
  { id: 'ga', label: 'Gestational age (weeks)', path: '%anc.gestationalAge.value', type: 'number' },
  { id: 'age', label: 'Age (years)', path: '%patient.age()', type: 'number' },
  { id: 'sex', label: 'Sex', path: '%patient.gender', type: 'code', values: ['female', 'male'] },
  { id: 'first', label: 'First pregnancy', path: '%anc.gravida = 1', type: 'boolean' },
]

export const PERIOD_DATES: PeriodDate[] = [
  { id: 'start', label: 'When the visit started (Encounter.period.start)', path: 'Encounter.period.start' },
  { id: 'lmp', label: 'Last menstrual period (Observation.value)', path: '%anc.lmp.value' },
  { id: 'recorded', label: 'When it was entered (meta.lastUpdated): not when care happened', path: 'meta.lastUpdated' },
]

const band = (label: string, lo: number, hi: number) => ({ label, test: (p: Patient) => p.age >= lo && p.age <= hi })
export const DISAGGREGATIONS: Disaggregation[] = [
  { id: 'moh-age-anc', label: 'MoH age bands, ANC (national)', shared: true, bands: [band('10–14', 10, 14), band('15–19', 15, 19), band('20–24', 20, 24), band('25+', 25, 200)] },
  { id: 'moh-sex', label: 'Sex (national)', shared: true, bands: [{ label: 'Female', test: (p) => p.sex === 'female' }, { label: 'Male', test: (p) => p.sex === 'male' }] },
  {
    id: 'first-pregnancy',
    label: 'First pregnancy (custom)',
    shared: false,
    bands: [
      { label: 'First', test: (p) => p.firstPregnancy },
      { label: 'Not first', test: (p) => !p.firstPregnancy },
    ],
  },
]

const c = (input: string, op: Op, value: string): Node => ({ kind: 'cond', input, op, value })
export const INITIAL: Indicator = {
  title: 'ANC 1st visit before 12 weeks',
  url: 'https://health.go.ug/fhir/Measure/anc-first-visit-12w',
  kind: 'proportion',
  periodDate: 'start',
  populations: [
    { key: 'initial', label: 'Initial population', tree: { kind: 'group', join: 'all', children: [c('sex', '=', 'female'), c('type', '=', 'anc-first')] } },
    { key: 'denominator', label: 'Denominator', tree: { kind: 'group', join: 'all', children: [] } },
    { key: 'exclusion', label: 'Denominator exclusion', tree: { kind: 'group', join: 'all', children: [c('age', '<', '10')] } },
    { key: 'numerator', label: 'Numerator', tree: { kind: 'group', join: 'all', children: [c('ga', '<', '12')] } },
  ],
  disaggregations: ['moh-age-anc'],
}

export const HMIS_MAPPING = 'HMIS 105 (2024) · 2.1 ANC 1st visit for women within 12 weeks · from Reporting package ug.moh.hmis 4.1.0'

export const PERIODS = ['2026-03', '2026-04']
export const PATIENTS: Patient[] = [
  { id: 'p1', name: 'Nakato A.', age: 23, sex: 'female', encounterType: 'anc-first', gestationWeeks: 10, firstPregnancy: true, start: '2026-03-04', recorded: '2026-03-04', lmp: '2025-12-24' },
  { id: 'p2', name: 'Akello B.', age: 17, sex: 'female', encounterType: 'anc-first', gestationWeeks: 16, firstPregnancy: true, start: '2026-03-11', recorded: '2026-03-11', lmp: '2025-11-19' },
  { id: 'p3', name: 'Namubiru C.', age: 31, sex: 'female', encounterType: 'anc-first', gestationWeeks: 9, firstPregnancy: false, start: '2026-03-30', recorded: '2026-04-03', lmp: '2026-01-26' },
  { id: 'p4', name: 'Auma D.', age: 28, sex: 'female', encounterType: 'anc-followup', gestationWeeks: 20, firstPregnancy: false, start: '2026-03-18', recorded: '2026-03-18', lmp: '2025-10-29' },
  { id: 'p5', name: 'Okello E.', age: 35, sex: 'male', encounterType: 'opd', gestationWeeks: 0, firstPregnancy: false, start: '2026-03-02', recorded: '2026-03-02', lmp: '' },
  { id: 'p6', name: 'Nabirye F.', age: 14, sex: 'female', encounterType: 'anc-first', gestationWeeks: 11, firstPregnancy: true, start: '2026-04-08', recorded: '2026-04-08', lmp: '2026-01-21' },
  { id: 'p7', name: 'Atim G.', age: 21, sex: 'female', encounterType: 'anc-first', gestationWeeks: 13, firstPregnancy: false, start: '2026-04-15', recorded: '2026-04-15', lmp: '2026-01-14' },
  { id: 'p8', name: 'Nansubuga H.', age: 26, sex: 'female', encounterType: 'anc-first', gestationWeeks: 8, firstPregnancy: true, start: '2026-03-25', recorded: '2026-03-25', lmp: '2026-01-28' },
]

const value = (p: Patient, input: string): string =>
  input === 'type' ? p.encounterType : input === 'ga' ? String(p.gestationWeeks) : input === 'age' ? String(p.age) : input === 'sex' ? p.sex : String(p.firstPregnancy)

function holdsCond(n: Extract<Node, { kind: 'cond' }>, p: Patient): boolean {
  const input = INPUTS.find((i) => i.id === n.input)
  const raw = value(p, n.input)
  if (input?.type === 'number') {
    const a = Number(raw)
    const b = Number(n.value)
    return n.op === '=' ? a === b : n.op === '!=' ? a !== b : n.op === '<' ? a < b : n.op === '<=' ? a <= b : n.op === '>' ? a > b : a >= b
  }
  return n.op === '!=' ? raw !== n.value : raw === n.value
}

export function holds(n: Node, p: Patient): boolean {
  if (n.kind === 'cond') return holdsCond(n, p)
  if (n.children.length === 0) return n.join !== 'any'
  if (n.join === 'all') return n.children.every((ch) => holds(ch, p))
  if (n.join === 'any') return n.children.some((ch) => holds(ch, p))
  return !n.children.some((ch) => holds(ch, p))
}

const lit = (input: string, v: string) => (INPUTS.find((i) => i.id === input)?.type === 'code' ? `'${v}'` : v)
export function fhirpath(n: Node): string {
  if (n.kind === 'cond') return `${INPUTS.find((i) => i.id === n.input)?.path ?? n.input} ${n.op} ${lit(n.input, n.value)}`
  if (n.children.length === 0) return n.join === 'any' ? 'false' : 'true'
  const parts = n.children.map((ch) => (ch.kind === 'group' && ch.children.length > 1 ? `(${fhirpath(ch)})` : fhirpath(ch)))
  if (n.join === 'not') return `(${parts.join(' or ')}).not()`
  return parts.join(n.join === 'all' ? ' and ' : ' or ')
}

export const condText = (n: Extract<Node, { kind: 'cond' }>) => `${INPUTS.find((i) => i.id === n.input)?.label ?? n.input} ${n.op} ${n.value}`

const dateOf = (p: Patient, d: PeriodDate['id']) => (d === 'start' ? p.start : d === 'recorded' ? p.recorded : p.lmp)

/** Which patients fall in each population for the period: the per-patient membership ADR 0068 keeps. */
export function evaluate(ind: Indicator, period: string) {
  const inPeriod = PATIENTS.filter((p) => dateOf(p, ind.periodDate).startsWith(period))
  const tree = (k: PopKey) => ind.populations.find((x) => x.key === k)?.tree ?? { kind: 'group' as const, join: 'all' as const, children: [] }
  const initial = inPeriod.filter((p) => holds(tree('initial'), p))
  if (ind.kind === 'count') return { initial, denominator: initial, exclusion: [] as Patient[], numerator: initial, inPeriod }
  const denomRaw = initial.filter((p) => holds(tree('denominator'), p))
  const exclusion = denomRaw.filter((p) => holds(tree('exclusion'), p))
  const denominator = denomRaw.filter((p) => !exclusion.includes(p))
  const numerator = denominator.filter((p) => holds(tree('numerator'), p))
  return { initial, denominator, exclusion, numerator, inPeriod }
}

export type Result = ReturnType<typeof evaluate>
export const pct = (r: Result) => (r.denominator.length ? Math.round((100 * r.numerator.length) / r.denominator.length) : 0)
