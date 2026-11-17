// PROTOTYPE (#534), throwaway: an indicator as numerator / denominator, where the denominator can
// be 1 (a count), and each side starts from something (everyone in the period, this indicator's
// denominator, or another indicator's numerator or denominator) and adds conditions. Sample test
// patients and a tiny evaluator; the real evaluation is the server's FHIRPath engine (ADR 0068).

export type Input = { id: string; label: string; path: string; type: 'code' | 'number' | 'boolean'; values?: string[] }
export type Op = '=' | '!=' | '<' | '<=' | '>' | '>='
export type Join = 'all' | 'any' | 'not'
export type Node = { kind: 'cond'; input: string; op: Op; value: string } | { kind: 'group'; join: Join; children: Node[] } | { kind: 'member'; ref: Base }
/** What a membership condition needs: the indicator being edited, the period, and the sides already being evaluated (to stop loops). */
export type Ctx = { ind: Indicator; period: string; seen: string[] }
export type Side = 'numerator' | 'denominator'
export type Base = { kind: 'period' } | { kind: 'denominator' } | { kind: 'indicator'; id: string; side: Side }
export type Part = { base: Base; tree: Node }
export type PeriodDate = { id: 'start' | 'recorded' | 'lmp'; label: string; path: string }
export type Disaggregation = { id: string; label: string; shared: boolean; bands: { label: string; test: (p: Patient) => boolean }[] }
export type Patient = { id: string; name: string; age: number; sex: 'female' | 'male'; encounterType: string; gestationWeeks: number; firstPregnancy: boolean; start: string; recorded: string; lmp: string }
export type Indicator = { id: string; title: string; version: string; periodDate: PeriodDate['id']; numerator: Part; denominator: Part | 'one'; disaggregations: string[] }
export type SavedTest = { period: string; expect: { numerator: number; denominator: number | 'one' } }

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
const all = (...children: Node[]): Node => ({ kind: 'group', join: 'all', children })
const everyone: Base = { kind: 'period' }

/** Published indicators in this package and its dependencies, which a side can start from. */
export const OTHERS: Indicator[] = [
  { id: 'anc-visits', title: 'ANC visits (all)', version: '2.3.0', periodDate: 'start', numerator: { base: everyone, tree: { kind: 'group', join: 'any', children: [c('type', '=', 'anc-first'), c('type', '=', 'anc-followup')] } }, denominator: 'one', disaggregations: [] },
  { id: 'anc-first', title: 'ANC 1st visits', version: '2.3.0', periodDate: 'start', numerator: { base: everyone, tree: all(c('sex', '=', 'female'), c('type', '=', 'anc-first')) }, denominator: 'one', disaggregations: ['moh-age-anc'] },
]

export const INITIAL: Indicator = {
  id: 'anc-first-12w',
  title: 'ANC 1st visit before 12 weeks',
  version: '2.4.0-draft',
  periodDate: 'start',
  denominator: { base: everyone, tree: all({ kind: 'member', ref: { kind: 'indicator', id: 'anc-first', side: 'numerator' } }, { kind: 'group', join: 'not', children: [c('age', '<', '10')] }) },
  numerator: { base: everyone, tree: all({ kind: 'member', ref: { kind: 'denominator' } }, c('ga', '<', '12')) },
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
  { id: 'p9', name: 'Achen I.', age: 9, sex: 'female', encounterType: 'anc-first', gestationWeeks: 10, firstPregnancy: true, start: '2026-03-20', recorded: '2026-03-20', lmp: '2026-01-09' },
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

export function holds(n: Node, p: Patient, ctx: Ctx): boolean {
  if (n.kind === 'cond') return holdsCond(n, p)
  if (n.kind === 'member') return baseOf(ctx.ind, n.ref, ctx.period, ctx.seen).some((x) => x.id === p.id)
  if (n.children.length === 0) return n.join !== 'any'
  if (n.join === 'all') return n.children.every((ch) => holds(ch, p, ctx))
  if (n.join === 'any') return n.children.some((ch) => holds(ch, p, ctx))
  return !n.children.some((ch) => holds(ch, p, ctx))
}

const lit = (input: string, v: string) => (INPUTS.find((i) => i.id === input)?.type === 'code' ? `'${v}'` : v)
export function fhirpath(n: Node, ind?: Indicator): string {
  if (n.kind === 'member') return `(${memberExpr(n.ref, ind)})`
  if (n.kind === 'cond') return `${INPUTS.find((i) => i.id === n.input)?.path ?? n.input} ${n.op} ${lit(n.input, n.value)}`
  if (n.children.length === 0) return n.join === 'any' ? 'false' : 'true'
  const parts = n.children.map((ch) => (ch.kind === 'group' && ch.children.length > 1 ? `(${fhirpath(ch, ind)})` : fhirpath(ch, ind)))
  if (n.join === 'not') return `(${parts.join(' or ')}).not()`
  return parts.join(n.join === 'all' ? ' and ' : ' or ')
}

export const condText = (n: Extract<Node, { kind: 'cond' }>) => `${INPUTS.find((i) => i.id === n.input)?.label ?? n.input} ${n.op} ${n.value}`

export const baseLabel = (b: Base) => {
  if (b.kind === 'period') return 'Everyone with care in the period'
  if (b.kind === 'denominator') return "This indicator's denominator"
  const o = OTHERS.find((x) => x.id === b.id)
  return `${o?.title ?? b.id} ${o?.version ?? ''}, its ${b.side}`
}

const dateOf = (p: Patient, d: PeriodDate['id']) => (d === 'start' ? p.start : d === 'recorded' ? p.recorded : p.lmp)

/** Who is in one side: start from its base, then keep those its own conditions hold for. */
export function side(ind: Indicator, s: Side, period: string, seen: string[] = []): Patient[] {
  if (seen.includes(`${ind.id}.${s}`)) return [] // a loop; the real check refuses it at Validation
  const part = s === 'numerator' ? ind.numerator : ind.denominator
  if (part === 'one') return []
  const next = [...seen, `${ind.id}.${s}`]
  const from = baseOf(ind, part.base, period, next)
  return from.filter((p) => holds(part.tree, p, { ind, period, seen: next }))
}

export function baseOf(ind: Indicator, b: Base, period: string, seen: string[] = []): Patient[] {
  if (b.kind === 'period') return PATIENTS.filter((p) => dateOf(p, ind.periodDate).startsWith(period))
  if (b.kind === 'denominator') return ind.denominator === 'one' ? baseOf(ind, { kind: 'period' }, period, seen) : side(ind, 'denominator', period, seen)
  const other = OTHERS.find((x) => x.id === b.id)
  return other ? side(other, b.side, period, seen) : []
}

/** The FHIRPath a side compiles to: its base's expression and its own conditions. */
export function compiled(ind: Indicator, s: Side): string {
  const part = s === 'numerator' ? ind.numerator : ind.denominator
  if (part === 'one') return '1'
  const own = fhirpath(part.tree, ind)
  const b = part.base
  const base = b.kind === 'period' ? '' : b.kind === 'denominator' ? (ind.denominator === 'one' ? '' : compiled(ind, 'denominator')) : (() => {
    const o = OTHERS.find((x) => x.id === b.id)
    return o ? `/* ${o.title}|${o.version} ${b.side} */ ${compiled(o, b.side)}` : ''
  })()
  return base ? `(${base}) and (${own})` : own
}

export function evaluate(ind: Indicator, period: string) {
  const numerator = side(ind, 'numerator', period)
  const denominator = ind.denominator === 'one' ? null : side(ind, 'denominator', period)
  return { numerator, denominator }
}
export type Result = ReturnType<typeof evaluate>
export const display = (r: Result) => (r.denominator === null ? `${r.numerator.length}` : r.denominator.length ? `${Math.round((100 * r.numerator.length) / r.denominator.length)}%` : '–')

/** The expression a membership condition stands for: the referenced side's own compiled criteria. */
function memberExpr(b: Base, ind?: Indicator): string {
  if (b.kind === 'period') return 'true'
  if (b.kind === 'denominator') return ind && ind.denominator !== 'one' ? `/* this indicator's denominator */ ${compiled(ind, 'denominator')}` : 'true'
  const o = OTHERS.find((x) => x.id === b.id)
  return o ? `/* ${o.title}|${o.version} ${b.side} */ ${compiled(o, b.side)}` : 'false'
}

/** The sides a membership condition may name. */
export function refOptions(): Base[] {
  const out: Base[] = []
  for (const o of OTHERS) {
    out.push({ kind: 'indicator', id: o.id, side: 'numerator' })
    if (o.denominator !== 'one') out.push({ kind: 'indicator', id: o.id, side: 'denominator' })
  }
  return out
}

/** Removes "is in the denominator" conditions, for when the denominator becomes 1. */
export function stripDenominator(n: Node): Node {
  if (n.kind !== 'group') return n
  return { ...n, children: n.children.filter((ch) => !(ch.kind === 'member' && ch.ref.kind === 'denominator')).map(stripDenominator) }
}
