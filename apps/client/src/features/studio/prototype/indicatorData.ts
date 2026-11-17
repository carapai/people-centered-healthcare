// PROTOTYPE (#534, #539), throwaway: an indicator as numerator / denominator (the denominator can
// be 1, a count), condition trees with value, date and "is in another indicator" conditions, date
// conditions relative to the reporting period, author-defined age bands and disaggregation by any
// coded value. Sample test patients and a tiny evaluator; the real evaluation is the server's
// FHIRPath engine (ADR 0068).

export type Input = { id: string; label: string; path: string; type: 'code' | 'number' | 'boolean'; values?: string[] }
export type Op = '=' | '!=' | '<' | '<=' | '>' | '>='
export type Join = 'all' | 'any' | 'not'
export type Side = 'numerator' | 'denominator'
export type Base = { kind: 'period' } | { kind: 'denominator' } | { kind: 'indicator'; id: string; side: Side }

// --- Dates ------------------------------------------------------------------------------------------

export type DateField = 'start' | 'obs' | 'enrolled' | 'eocStart' | 'eocEnd' | 'lmp' | 'dob' | 'nextVisit' | 'recorded'
export const DATE_FIELDS: { id: DateField; label: string; path: string }[] = [
  { id: 'start', label: 'Visit started', path: 'Encounter.period.start' },
  { id: 'obs', label: 'Observation happened', path: '%obs.effective' },
  { id: 'enrolled', label: 'Program enrollment', path: '%enrollment.period.start' },
  { id: 'eocStart', label: 'Episode of care start', path: '%episode.period.start' },
  { id: 'eocEnd', label: 'Episode of care end', path: '%episode.period.end' },
  { id: 'lmp', label: 'Last menstrual period', path: '%anc.lmp.value' },
  { id: 'dob', label: 'Date of birth', path: '%patient.birthDate' },
  { id: 'nextVisit', label: 'Next appointment', path: '%appointment.start' },
  { id: 'recorded', label: 'Entered (not when care happened)', path: 'meta.lastUpdated' },
]
/** A date relative to the reporting period, or a fixed one. */
export type Anchor = { at: 'periodStart' | 'periodEnd' | 'fixed'; date?: string }
export type DateOp = 'inPeriod' | 'on' | 'before' | 'after' | 'between' | 'within' | 'empty' | 'notEmpty' | 'previous'
export const DATE_OPS: { id: DateOp; label: string }[] = [
  { id: 'inPeriod', label: 'is in the reporting period' },
  { id: 'on', label: 'is on' },
  { id: 'before', label: 'is before' },
  { id: 'after', label: 'is after' },
  { id: 'between', label: 'is between' },
  { id: 'within', label: 'is within' },
  { id: 'previous', label: 'is in the previous' },
  { id: 'empty', label: 'is empty' },
  { id: 'notEmpty', label: 'is not empty' },
]
export type Unit = 'days' | 'weeks' | 'months'
export type DateCond = { kind: 'date'; field: DateField; op: DateOp; a: Anchor; b: Anchor; n: number; unit: Unit; dir: 'before' | 'after' }

export type Node = { kind: 'cond'; input: string; op: Op; value: string } | { kind: 'group'; join: Join; children: Node[] } | { kind: 'member'; ref: Base } | DateCond
/** What evaluation needs: the indicator being edited, the period, and the sides already being evaluated (to stop loops). */
export type Ctx = { ind: Indicator; period: string; seen: string[] }

// --- Disaggregations ---------------------------------------------------------------------------------

export type AgeBand = { from: number; to: number | null }
export type NationalId = 'moh-age-anc' | 'moh-age-hmis'
export type Disagg = { kind: 'national'; id: NationalId } | { kind: 'age'; label: string; unit: 'years' | 'months'; at: 'periodEnd' | 'periodDate'; bands: AgeBand[] } | { kind: 'coded'; input: string }
export const NATIONAL: Record<NationalId, { label: string; bands: AgeBand[] }> = {
  'moh-age-anc': { label: 'MoH age bands, ANC', bands: [{ from: 10, to: 14 }, { from: 15, to: 19 }, { from: 20, to: 24 }, { from: 25, to: null }] },
  'moh-age-hmis': { label: 'MoH age bands, HMIS 105', bands: [{ from: 0, to: 0 }, { from: 1, to: 4 }, { from: 5, to: 9 }, { from: 10, to: 19 }, { from: 20, to: null }] },
}

export type Patient = {
  id: string
  name: string
  dob: string
  sex: 'female' | 'male'
  encounterType: string
  gestationWeeks: number
  firstPregnancy: boolean
  hiv: 'positive' | 'negative' | 'unknown'
  district: string
  dates: Partial<Record<DateField, string>>
}
export type Indicator = { id: string; title: string; version: string; periodDate: DateField; numerator: { tree: Node }; denominator: { tree: Node } | 'one'; disaggregations: Disagg[] }
export type SavedTest = { period: string; expect: { numerator: number; denominator: number | 'one' } }

export const INPUTS: Input[] = [
  { id: 'type', label: 'Visit type', path: '%encounter.type.coding.code', type: 'code', values: ['anc-first', 'anc-followup', 'opd'] },
  { id: 'ga', label: 'Gestational age (weeks)', path: '%anc.gestationalAge.value', type: 'number' },
  { id: 'age', label: 'Age (years)', path: '%patient.age()', type: 'number' },
  { id: 'sex', label: 'Sex', path: '%patient.gender', type: 'code', values: ['female', 'male'] },
  { id: 'hiv', label: 'HIV status', path: '%hiv.value.coding.code', type: 'code', values: ['positive', 'negative', 'unknown'] },
  { id: 'district', label: 'District of residence', path: '%patient.address.district', type: 'code', values: ['Kampala', 'Wakiso', 'Mukono'] },
  { id: 'first', label: 'First pregnancy', path: '%anc.gravida = 1', type: 'boolean' },
]
export const CODED = INPUTS.filter((i) => i.type === 'code' || i.type === 'boolean')

const c = (input: string, op: Op, value: string): Node => ({ kind: 'cond', input, op, value })
const all = (...children: Node[]): Node => ({ kind: 'group', join: 'all', children })
const inPeriod = (field: DateField): DateCond => ({ kind: 'date', field, op: 'inPeriod', a: { at: 'periodStart' }, b: { at: 'periodEnd' }, n: 3, unit: 'months', dir: 'before' })
export const newDate = (): DateCond => inPeriod('start')

/** Published indicators in this package and its dependencies, which a condition can refer to. */
export const OTHERS: Indicator[] = [
  { id: 'anc-visits', title: 'ANC visits (all)', version: '2.3.0', periodDate: 'start', numerator: { tree: { kind: 'group', join: 'any', children: [c('type', '=', 'anc-first'), c('type', '=', 'anc-followup')] } }, denominator: 'one', disaggregations: [] },
  { id: 'anc-first', title: 'ANC 1st visits', version: '2.3.0', periodDate: 'start', numerator: { tree: all(c('sex', '=', 'female'), c('type', '=', 'anc-first')) }, denominator: 'one', disaggregations: [] },
]

export const INITIAL: Indicator = {
  id: 'anc-first-12w',
  title: 'ANC 1st visit before 12 weeks',
  version: '2.4.0-draft',
  periodDate: 'start',
  denominator: { tree: all({ kind: 'member', ref: { kind: 'indicator', id: 'anc-first', side: 'numerator' } }, { kind: 'group', join: 'not', children: [c('age', '<', '10')] }) },
  numerator: { tree: all(c('ga', '<', '12')) },
  disaggregations: [{ kind: 'national', id: 'moh-age-anc' }, { kind: 'coded', input: 'hiv' }],
}

export const HMIS_MAPPING = 'HMIS 105 (2024) · 2.1 ANC 1st visit for women within 12 weeks · from Reporting package ug.moh.hmis 4.1.0'
export const PERIODS = ['2026-02', '2026-03', '2026-04']

const P = (id: string, name: string, dob: string, sex: Patient['sex'], encounterType: string, ga: number, first: boolean, hiv: Patient['hiv'], district: string, dates: Patient['dates']): Patient => ({ id, name, dob, sex, encounterType, gestationWeeks: ga, firstPregnancy: first, hiv, district, dates: { dob, ...dates } })
export const PATIENTS: Patient[] = [
  P('p1', 'Nakato A.', '2002-11-02', 'female', 'anc-first', 10, true, 'negative', 'Kampala', { start: '2026-03-04', obs: '2026-03-04', recorded: '2026-03-04', lmp: '2025-12-24', enrolled: '2026-03-04', eocStart: '2026-03-04', nextVisit: '2026-04-01' }),
  P('p2', 'Akello B.', '2008-06-15', 'female', 'anc-first', 16, true, 'unknown', 'Wakiso', { start: '2026-03-11', obs: '2026-03-11', recorded: '2026-03-11', lmp: '2025-11-19', enrolled: '2026-03-11', eocStart: '2026-03-11' }),
  P('p3', 'Namubiru C.', '1994-09-30', 'female', 'anc-first', 9, false, 'positive', 'Kampala', { start: '2026-03-30', obs: '2026-03-30', recorded: '2026-04-03', lmp: '2026-01-26', enrolled: '2026-03-30', eocStart: '2026-03-30', nextVisit: '2026-04-27' }),
  P('p4', 'Auma D.', '1997-12-01', 'female', 'anc-followup', 20, false, 'negative', 'Mukono', { start: '2026-03-18', obs: '2026-03-18', recorded: '2026-03-18', lmp: '2025-10-29', enrolled: '2026-01-07', eocStart: '2026-01-07' }),
  P('p5', 'Okello E.', '1990-04-22', 'male', 'opd', 0, false, 'negative', 'Kampala', { start: '2026-03-02', obs: '2026-03-02', recorded: '2026-03-02' }),
  P('p6', 'Nabirye F.', '2011-08-09', 'female', 'anc-first', 11, true, 'negative', 'Wakiso', { start: '2026-04-08', obs: '2026-04-08', recorded: '2026-04-08', lmp: '2026-01-21', enrolled: '2026-04-08', eocStart: '2026-04-08' }),
  P('p7', 'Atim G.', '2004-12-19', 'female', 'anc-first', 13, false, 'positive', 'Mukono', { start: '2026-04-15', obs: '2026-04-15', recorded: '2026-04-15', lmp: '2026-01-14', enrolled: '2026-04-15', eocStart: '2026-04-15' }),
  P('p8', 'Nansubuga H.', '1999-07-03', 'female', 'anc-first', 8, true, 'negative', 'Kampala', { start: '2026-03-25', obs: '2026-03-25', recorded: '2026-03-25', lmp: '2026-01-28', enrolled: '2026-03-25', eocStart: '2026-03-25' }),
  P('p9', 'Achen I.', '2016-10-10', 'female', 'anc-first', 10, true, 'unknown', 'Wakiso', { start: '2026-03-20', obs: '2026-03-20', recorded: '2026-03-20', lmp: '2026-01-09', enrolled: '2026-03-20', eocStart: '2026-03-20' }),
  P('p10', 'Kisakye J.', '1996-02-14', 'female', 'anc-first', 11, false, 'negative', 'Kampala', { start: '2026-02-17', obs: '2026-02-17', recorded: '2026-02-17', lmp: '2025-12-02', enrolled: '2026-02-17', eocStart: '2026-02-17', eocEnd: '2026-03-12' }),
]

// --- Period arithmetic (monthly periods "YYYY-MM") -----------------------------------------------------

const iso = (d: Date) => d.toISOString().slice(0, 10)
export const periodStart = (p: string) => `${p}-01`
export const periodEnd = (p: string) => {
  const [y = 0, m = 1] = p.split('-').map(Number)
  return iso(new Date(Date.UTC(y, m, 0)))
}
const shift = (date: string, n: number, unit: Unit) => {
  const d = new Date(`${date}T00:00:00Z`)
  if (unit === 'days') d.setUTCDate(d.getUTCDate() + n)
  else if (unit === 'weeks') d.setUTCDate(d.getUTCDate() + 7 * n)
  else d.setUTCMonth(d.getUTCMonth() + n)
  return iso(d)
}
const anchor = (a: Anchor, period: string) => (a.at === 'periodStart' ? periodStart(period) : a.at === 'periodEnd' ? periodEnd(period) : (a.date ?? ''))
export const ageAt = (dob: string, at: string, unit: 'years' | 'months') => {
  const [by = 0, bm = 1, bd = 1] = dob.split('-').map(Number)
  const [ay = 0, am = 1, ad = 1] = at.split('-').map(Number)
  const months = (ay - by) * 12 + (am - bm) - (ad < bd ? 1 : 0)
  return unit === 'months' ? months : Math.floor(months / 12)
}

// --- Evaluation ----------------------------------------------------------------------------------------

const value = (p: Patient, input: string, period: string): string =>
  input === 'type' ? p.encounterType : input === 'ga' ? String(p.gestationWeeks) : input === 'age' ? String(ageAt(p.dob, periodEnd(period), 'years')) : input === 'sex' ? p.sex : input === 'hiv' ? p.hiv : input === 'district' ? p.district : String(p.firstPregnancy)

function holdsCond(n: Extract<Node, { kind: 'cond' }>, p: Patient, period: string): boolean {
  const input = INPUTS.find((i) => i.id === n.input)
  const raw = value(p, n.input, period)
  if (input?.type === 'number') {
    const a = Number(raw)
    const b = Number(n.value)
    return n.op === '=' ? a === b : n.op === '!=' ? a !== b : n.op === '<' ? a < b : n.op === '<=' ? a <= b : n.op === '>' ? a > b : a >= b
  }
  return n.op === '!=' ? raw !== n.value : raw === n.value
}

function holdsDate(n: DateCond, p: Patient, period: string): boolean {
  const d = p.dates[n.field]
  if (n.op === 'empty') return !d
  if (n.op === 'notEmpty') return Boolean(d)
  if (!d) return false
  const a = anchor(n.a, period)
  const b = anchor(n.b, period)
  if (n.op === 'inPeriod') return d >= periodStart(period) && d <= periodEnd(period)
  if (n.op === 'on') return d === a
  if (n.op === 'before') return d < a
  if (n.op === 'after') return d > a
  if (n.op === 'between') return d >= a && d <= b
  if (n.op === 'previous') return d >= shift(periodStart(period), -n.n, 'months') && d < periodStart(period)
  const other = shift(a, n.dir === 'before' ? -n.n : n.n, n.unit)
  return n.dir === 'before' ? d >= other && d <= a : d >= a && d <= other
}

export function holds(n: Node, p: Patient, ctx: Ctx): boolean {
  if (n.kind === 'cond') return holdsCond(n, p, ctx.period)
  if (n.kind === 'date') return holdsDate(n, p, ctx.period)
  if (n.kind === 'member') return baseOf(ctx.ind, n.ref, ctx.period, ctx.seen).some((x) => x.id === p.id)
  if (n.children.length === 0) return n.join !== 'any'
  if (n.join === 'all') return n.children.every((ch) => holds(ch, p, ctx))
  if (n.join === 'any') return n.children.some((ch) => holds(ch, p, ctx))
  return !n.children.some((ch) => holds(ch, p, ctx))
}

/** Where a side starts: the numerator is always within the denominator when there is one. */
export const startOf = (ind: Indicator, s: Side): Base => (s === 'numerator' && ind.denominator !== 'one' ? { kind: 'denominator' } : { kind: 'period' })

export function side(ind: Indicator, s: Side, period: string, seen: string[] = []): Patient[] {
  if (seen.includes(`${ind.id}.${s}`)) return [] // a loop; the real check refuses it at Validation
  const part = s === 'numerator' ? ind.numerator : ind.denominator
  if (part === 'one') return []
  const next = [...seen, `${ind.id}.${s}`]
  return baseOf(ind, startOf(ind, s), period, next).filter((p) => holds(part.tree, p, { ind, period, seen: next }))
}

export function baseOf(ind: Indicator, b: Base, period: string, seen: string[] = []): Patient[] {
  if (b.kind === 'period') return PATIENTS.filter((p) => holdsDate(inPeriod(ind.periodDate), p, period))
  if (b.kind === 'denominator') return ind.denominator === 'one' ? baseOf(ind, { kind: 'period' }, period, seen) : side(ind, 'denominator', period, seen)
  const other = OTHERS.find((x) => x.id === b.id)
  return other ? side(other, b.side, period, seen) : []
}

export function evaluate(ind: Indicator, period: string) {
  return { numerator: side(ind, 'numerator', period), denominator: ind.denominator === 'one' ? null : side(ind, 'denominator', period) }
}
export type Result = ReturnType<typeof evaluate>
export const display = (r: Result) => (r.denominator === null ? `${r.numerator.length}` : r.denominator.length ? `${Math.round((100 * r.numerator.length) / r.denominator.length)}%` : '–')

// --- Bands for the Try table ---------------------------------------------------------------------------

export const bandLabel = (b: AgeBand, unit: string) => (b.to === null ? `${b.from}+ ${unit}` : b.from === b.to ? `${b.from} ${unit}` : `${b.from}–${b.to} ${unit}`)
export const disaggLabel = (d: Disagg) => (d.kind === 'national' ? `${NATIONAL[d.id].label} (national)` : d.kind === 'age' ? `${d.label} (age bands)` : `${INPUTS.find((i) => i.id === d.input)?.label ?? d.input} (coded)`)

export function bands(d: Disagg, ind: Indicator, period: string): { label: string; test: (p: Patient) => boolean }[] {
  const ageBands = (list: AgeBand[], unit: 'years' | 'months', at: 'periodEnd' | 'periodDate') =>
    list.map((b) => ({
      label: bandLabel(b, unit),
      test: (p: Patient) => {
        const age = ageAt(p.dob, at === 'periodEnd' ? periodEnd(period) : (p.dates[ind.periodDate] ?? periodEnd(period)), unit)
        return age >= b.from && (b.to === null || age <= b.to)
      },
    }))
  if (d.kind === 'national') return ageBands(NATIONAL[d.id].bands, 'years', 'periodEnd')
  if (d.kind === 'age') return ageBands(d.bands, d.unit, d.at)
  const input = INPUTS.find((i) => i.id === d.input)
  const values = input?.type === 'boolean' ? ['true', 'false'] : (input?.values ?? [])
  return values.map((v) => ({ label: v, test: (p: Patient) => value(p, d.input, period) === v }))
}

// --- FHIRPath ------------------------------------------------------------------------------------------

const lit = (input: string, v: string) => (INPUTS.find((i) => i.id === input)?.type === 'code' ? `'${v}'` : v)
const anchorExpr = (a: Anchor) => (a.at === 'periodStart' ? '%periodStart' : a.at === 'periodEnd' ? '%periodEnd' : `@${a.date ?? ''}`)
function dateExpr(n: DateCond): string {
  const f = DATE_FIELDS.find((x) => x.id === n.field)?.path ?? n.field
  if (n.op === 'inPeriod') return `${f} >= %periodStart and ${f} <= %periodEnd`
  if (n.op === 'on') return `${f} = ${anchorExpr(n.a)}`
  if (n.op === 'before') return `${f} < ${anchorExpr(n.a)}`
  if (n.op === 'after') return `${f} > ${anchorExpr(n.a)}`
  if (n.op === 'between') return `${f} >= ${anchorExpr(n.a)} and ${f} <= ${anchorExpr(n.b)}`
  if (n.op === 'previous') return `${f} >= %periodStart - ${n.n} months and ${f} < %periodStart`
  if (n.op === 'empty') return `${f}.empty()`
  if (n.op === 'notEmpty') return `${f}.exists()`
  return n.dir === 'before' ? `${f} >= ${anchorExpr(n.a)} - ${n.n} ${n.unit} and ${f} <= ${anchorExpr(n.a)}` : `${f} >= ${anchorExpr(n.a)} and ${f} <= ${anchorExpr(n.a)} + ${n.n} ${n.unit}`
}

export function fhirpath(n: Node, ind?: Indicator): string {
  if (n.kind === 'member') return `(${memberExpr(n.ref, ind)})`
  if (n.kind === 'date') return `(${dateExpr(n)})`
  if (n.kind === 'cond') return `${INPUTS.find((i) => i.id === n.input)?.path ?? n.input} ${n.op} ${lit(n.input, n.value)}`
  if (n.children.length === 0) return n.join === 'any' ? 'false' : 'true'
  const parts = n.children.map((ch) => (ch.kind === 'group' && ch.children.length > 1 ? `(${fhirpath(ch, ind)})` : fhirpath(ch, ind)))
  if (n.join === 'not') return `(${parts.join(' or ')}).not()`
  return parts.join(n.join === 'all' ? ' and ' : ' or ')
}

export function compiled(ind: Indicator, s: Side): string {
  const part = s === 'numerator' ? ind.numerator : ind.denominator
  if (part === 'one') return '1'
  const own = fhirpath(part.tree, ind)
  const base = startOf(ind, s).kind === 'denominator' ? compiled(ind, 'denominator') : dateExpr(inPeriod(ind.periodDate))
  return `(${base}) and (${own})`
}

function memberExpr(b: Base, ind?: Indicator): string {
  if (b.kind === 'period') return 'true'
  if (b.kind === 'denominator') return ind && ind.denominator !== 'one' ? `/* this indicator's denominator */ ${compiled(ind, 'denominator')}` : 'true'
  const o = OTHERS.find((x) => x.id === b.id)
  return o ? `/* ${o.title}|${o.version} ${b.side} */ ${compiled(o, b.side)}` : 'false'
}

export const baseLabel = (b: Base) => {
  if (b.kind === 'period') return 'Everyone with care in the period'
  if (b.kind === 'denominator') return "This indicator's denominator"
  const o = OTHERS.find((x) => x.id === b.id)
  return `${o?.title ?? b.id} ${o?.version ?? ''}, its ${b.side}`
}

/** The sides an "is in another indicator" condition may name. */
export function refOptions(): Base[] {
  const out: Base[] = []
  for (const o of OTHERS) {
    out.push({ kind: 'indicator', id: o.id, side: 'numerator' })
    if (o.denominator !== 'one') out.push({ kind: 'indicator', id: o.id, side: 'denominator' })
  }
  return out
}
