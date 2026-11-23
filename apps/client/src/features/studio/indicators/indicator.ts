// An indicator in the Studio (studio.md § Indicators; analytics.md; ADR 0068; #537), as pure
// functions for the indicator editor.
//
// An indicator is a FHIR R5 `Measure` (`health-analytics-app`'s `measure` reads it):
//
// - **What it calculates** is its scoring: a count is `cohort`, a percentage `proportion`, a rate
//   or a ratio `ratio` (a rate with its factor in `indicator-rate-per`), and a sum, average,
//   median, minimum, maximum or distinct count `continuous-variable`, its `measure-observation`
//   naming the `aggregateMethod` and the value (`criteria`, or `indicator-value-between` two
//   dates) and `indicator-value-pick` taking each person's or enrollment's first or last.
// - **What is counted** is `indicator-counts` (people, enrollments, visits or observations), the
//   record type its criteria read its `basis`, and the declared date that places care in a
//   Report's period its `indicator-period-date`. `indicator-cumulative` counts up to the period's
//   end.
// - **The numerator** is the count's `initial-population`, a percentage's, rate's or ratio's
//   `numerator` (within its `denominator`), or a value's `measure-population`; each is a condition
//   tree (`conditions.ts`) saved as one FHIRPath expression.
// - **Disaggregations** are the first group's stratifiers: an Age Band Set pinned to
//   `url|version` (`indicator-age-band-set`), or a coded value (`criteria`) named by its `code`.
//
// The editor reads what it edits and writes it back, keeping everything else of the Measure as
// read (other extensions, descriptions, populations' ids). Validation remains the authority; the
// problems here mirror what it refuses so the editor can say so before saving.

import { compileCriteria, parseCriteria, referencesOf, type Criteria, type CriteriaNode } from './conditions'

export const EXTENSION = 'https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition'
export const PERIOD_DATE = `${EXTENSION}/indicator-period-date`
export const COUNTS = `${EXTENSION}/indicator-counts`
export const RATE_PER = `${EXTENSION}/indicator-rate-per`
export const CUMULATIVE = `${EXTENSION}/indicator-cumulative`
export const VALUE_PICK = `${EXTENSION}/indicator-value-pick`
export const VALUE_BETWEEN = `${EXTENSION}/indicator-value-between`
export const AGE_BAND_SET = `${EXTENSION}/indicator-age-band-set`
const OWN_EXTENSIONS = [PERIOD_DATE, COUNTS, RATE_PER, CUMULATIVE]

const SCORING = 'http://terminology.hl7.org/CodeSystem/measure-scoring'
const POPULATION = 'http://terminology.hl7.org/CodeSystem/measure-population'
const AGGREGATE = 'http://terminology.hl7.org/CodeSystem/measure-aggregate-method'

export const CALCULATIONS = ['count', 'percentage', 'rate', 'ratio', 'sum', 'average', 'median', 'minimum', 'maximum', 'distinct-count'] as const
export type Calculation = (typeof CALCULATIONS)[number]
export const COUNTED = ['people', 'enrollments', 'visits', 'observations'] as const
export type Counted = (typeof COUNTED)[number]
export const RATE_FACTORS = [100, 1000, 10000, 100000] as const
export const BETWEEN_UNITS = ['days', 'weeks', 'months', 'years'] as const
export type BetweenUnit = (typeof BETWEEN_UNITS)[number]
export const BASES = ['Encounter', 'Observation', 'EpisodeOfCare', 'Condition', 'Procedure', 'MedicationDispense', 'Immunization'] as const

/** Whether `calculation` has a denominator. */
export const hasDenominator = (calculation: Calculation) => calculation === 'percentage' || calculation === 'rate' || calculation === 'ratio'
/** Whether `calculation` takes a value of each counted thing. */
export const takesValue = (calculation: Calculation) => !hasDenominator(calculation) && calculation !== 'count'

/** The value a sum, average, median, minimum, maximum or distinct count takes. */
export type IndicatorValue = { kind: 'expression'; expression: string } | { kind: 'between'; from: string; to: string; unit: BetweenUnit }

/** A Disaggregation: by an Age Band Set (`url|version`), or by the code a FHIRPath gives. */
export type Disaggregation = { kind: 'age'; set: string } | { kind: 'coded'; name: string; criteria: string }

/** What the editor edits. */
export interface Indicator {
  title: string
  description: string
  basis: string
  periodDate: string
  counts: Counted
  calculation: Calculation
  /** A rate's factor. */
  ratePer: number
  cumulative: boolean
  numerator: Criteria
  /** A percentage's, rate's or ratio's; null otherwise. */
  denominator: Criteria | null
  /** A value calculation's value. */
  value: IndicatorValue
  pick: 'first' | 'last'
  disaggregations: Disaggregation[]
}

type Json = Record<string, unknown>

const isObject = (value: unknown): value is Json => typeof value === 'object' && value !== null && !Array.isArray(value)
const list = (value: unknown): unknown[] => (Array.isArray(value) ? value : [])
const string = (value: unknown): string => (typeof value === 'string' ? value : '')
const codeOf = (concept: unknown): string => {
  if (!isObject(concept)) return ''
  for (const c of list(concept['coding'])) if (isObject(c) && typeof c['code'] === 'string') return c['code']
  return ''
}
const extensionOf = (resource: Json, url: string): Json | undefined => list(resource['extension']).find((e): e is Json => isObject(e) && e['url'] === url)
const expressionOf = (value: unknown): string => (isObject(value) ? string(value['expression']) : '')

/** Whether `value` is a Measure. */
export const isMeasure = (value: unknown): value is Json => isObject(value) && value['resourceType'] === 'Measure'

const EVERYONE: Criteria = { kind: 'group', join: 'all', children: [] }

/** The populations of the Measure's first group, by code. */
function populations(resource: Json): Map<string, Json> {
  const group = list(resource['group'])[0]
  const found = new Map<string, Json>()
  for (const p of isObject(group) ? list(group['population']) : []) if (isObject(p)) found.set(codeOf(p['code']), p)
  return found
}

function calculationOf(resource: Json, observation: Json | undefined): Calculation {
  switch (codeOf(resource['scoring'])) {
    case 'proportion':
      return 'percentage'
    case 'ratio':
      return extensionOf(resource, RATE_PER) === undefined ? 'ratio' : 'rate'
    case 'continuous-variable': {
      const method = codeOf(observation?.['aggregateMethod'])
      return CALCULATIONS.find((c) => c === method && takesValue(c)) ?? 'sum'
    }
    default:
      return 'count'
  }
}

/** The indicator a Measure holds, or why the editor cannot read it. */
export function readIndicator(text: string): { measure: Json; indicator: Indicator } | { refusal: 'not-json' | 'not-measure' } {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return { refusal: 'not-json' }
  }
  if (!isMeasure(value)) return { refusal: 'not-measure' }
  const found = populations(value)
  const observation = found.get('measure-observation')
  const calculation = calculationOf(value, observation)
  const criteria = (code: string) => parseCriteria(expressionOf(found.get(code)?.['criteria']) || 'true')
  const numerator = hasDenominator(calculation) ? criteria('numerator') : takesValue(calculation) ? criteria('measure-population') : criteria('initial-population')
  const between = observation === undefined ? undefined : extensionOf(observation, VALUE_BETWEEN)
  const sub = (url: string) => (between === undefined ? undefined : extensionOf(between, url))
  const taken: IndicatorValue =
    between === undefined
      ? { kind: 'expression', expression: expressionOf(observation?.['criteria']) }
      : {
          kind: 'between',
          from: expressionOf(sub('from')?.['valueExpression']),
          to: expressionOf(sub('to')?.['valueExpression']),
          unit: BETWEEN_UNITS.find((u) => u === sub('unit')?.['valueCode']) ?? 'days',
        }
  const group = list(value['group'])[0]
  const disaggregations = (isObject(group) ? list(group['stratifier']) : []).flatMap((s): Disaggregation[] => {
    if (!isObject(s)) return []
    const age = extensionOf(s, AGE_BAND_SET)
    if (age !== undefined) return [{ kind: 'age', set: string(age['valueCanonical']) }]
    const code = isObject(s['code']) ? codeOf(s['code']) || string(s['code']['text']) : ''
    return [{ kind: 'coded', name: code, criteria: expressionOf(s['criteria']) }]
  })
  const rate = extensionOf(value, RATE_PER)?.['valueInteger']
  return {
    measure: value,
    indicator: {
      title: string(value['title']) || string(value['name']),
      description: string(value['description']),
      basis: string(value['basis']) || 'Encounter',
      periodDate: expressionOf(extensionOf(value, PERIOD_DATE)?.['valueExpression']),
      counts: COUNTED.find((c) => c === extensionOf(value, COUNTS)?.['valueCode']) ?? 'people',
      calculation,
      ratePer: typeof rate === 'number' ? rate : 1000,
      cumulative: extensionOf(value, CUMULATIVE)?.['valueBoolean'] === true,
      numerator,
      denominator: hasDenominator(calculation) ? criteria('denominator') : null,
      value: taken,
      pick: observation !== undefined && extensionOf(observation, VALUE_PICK)?.['valueCode'] === 'last' ? 'last' : 'first',
      disaggregations,
    },
  }
}

// ---------------------------------------------------------------------------------------------
// Changing what it calculates

/** `indicator` calculating `calculation`: a denominator appears (everyone, to be narrowed) for a
 * percentage, rate or ratio, and goes for the others. */
export function withCalculation(indicator: Indicator, calculation: Calculation): Indicator {
  const denominator = hasDenominator(calculation) ? (indicator.denominator ?? EVERYONE) : null
  return { ...indicator, calculation, denominator }
}

/** *+ Add denominator*: a percentage of everyone with care, narrowed by the author. */
export const addDenominator = (indicator: Indicator) => withCalculation(indicator, hasDenominator(indicator.calculation) ? indicator.calculation : 'percentage')

/** *Remove denominator*: a count of the numerator's conditions. */
export const removeDenominator = (indicator: Indicator) => withCalculation(indicator, 'count')

// ---------------------------------------------------------------------------------------------
// Writing

function scoringOf(calculation: Calculation): string {
  if (calculation === 'count') return 'cohort'
  if (calculation === 'percentage') return 'proportion'
  if (calculation === 'rate' || calculation === 'ratio') return 'ratio'
  return 'continuous-variable'
}

const fhirpath = (expression: string, description?: string) => ({ language: 'text/fhirpath', expression, ...(description === undefined || description === '' ? {} : { description }) })

/** The populations `indicator` needs, in order, each with its criteria; an existing one keeps its
 * id and description. */
function writePopulations(indicator: Indicator, held: Map<string, Json>): Json[] {
  const population = (code: string, expression: string, extra: Json = {}): Json => {
    const before = held.get(code) ?? {}
    const { criteria: _criteria, aggregateMethod: _method, extension: _extension, inputPopulationId: _input, ...kept } = before
    const criteria = isObject(before['criteria']) ? { ...before['criteria'], ...fhirpath(expression) } : fhirpath(expression)
    return { id: code, ...kept, code: { coding: [{ system: POPULATION, code }] }, criteria, ...extra }
  }
  const numerator = compileCriteria(indicator.numerator)
  if (indicator.calculation === 'count') return [population('initial-population', numerator)]
  const everyone = population('initial-population', 'true')
  if (hasDenominator(indicator.calculation)) return [everyone, population('denominator', compileCriteria(indicator.denominator ?? EVERYONE)), population('numerator', numerator)]
  const value = indicator.value
  const observation = population('measure-observation', value.kind === 'expression' ? value.expression : '', {
    extension: [
      ...(value.kind === 'between'
        ? [
            {
              url: VALUE_BETWEEN,
              extension: [
                { url: 'from', valueExpression: fhirpath(value.from) },
                { url: 'to', valueExpression: fhirpath(value.to) },
                { url: 'unit', valueCode: value.unit },
              ],
            },
          ]
        : []),
      ...(indicator.counts === 'people' || indicator.counts === 'enrollments' ? [{ url: VALUE_PICK, valueCode: indicator.pick }] : []),
    ],
    inputPopulationId: 'measure-population',
    aggregateMethod: { coding: [{ system: AGGREGATE, code: indicator.calculation }] },
  })
  if (value.kind === 'between') delete observation['criteria']
  if ((observation['extension'] as unknown[]).length === 0) delete observation['extension']
  return [everyone, population('measure-population', numerator), observation]
}

function writeStratifiers(indicator: Indicator, held: unknown[]): Json[] {
  return indicator.disaggregations.map((d) => {
    if (d.kind === 'age') {
      const before = held.find((s) => isObject(s) && extensionOf(s, AGE_BAND_SET)?.['valueCanonical'] === d.set)
      const description = isObject(before) && typeof before['description'] === 'string' ? { description: before['description'] } : {}
      return { extension: [{ url: AGE_BAND_SET, valueCanonical: d.set }], ...description }
    }
    const before = held.find((s) => isObject(s) && extensionOf(s, AGE_BAND_SET) === undefined && isObject(s['code']) && (codeOf(s['code']) || string(s['code']['text'])) === d.name)
    const description = isObject(before) && typeof before['description'] === 'string' ? { description: before['description'] } : {}
    return { code: { coding: [{ code: d.name }] }, criteria: fhirpath(d.criteria), ...description }
  })
}

/** `measure` holding `indicator`, everything it does not edit as read. */
export function writeIndicator(measure: Json, indicator: Indicator): Json {
  const extensions = list(measure['extension']).filter((e) => !(isObject(e) && OWN_EXTENSIONS.includes(string(e['url']))))
  const periodBefore = extensionOf(measure, PERIOD_DATE)?.['valueExpression']
  const periodDescription = isObject(periodBefore) ? string(periodBefore['description']) : ''
  const own: Json[] = [
    { url: PERIOD_DATE, valueExpression: fhirpath(indicator.periodDate, periodDescription) },
    { url: COUNTS, valueCode: indicator.counts },
    ...(indicator.calculation === 'rate' ? [{ url: RATE_PER, valueInteger: indicator.ratePer }] : []),
    ...(indicator.cumulative ? [{ url: CUMULATIVE, valueBoolean: true }] : []),
  ]
  const groups = list(measure['group'])
  const first: Json = isObject(groups[0]) ? groups[0] : {}
  const stratifiers = writeStratifiers(indicator, list(first['stratifier']))
  const { stratifier: _stratifier, ...group } = first
  const { description: _description, ...rest } = measure
  return {
    ...rest,
    title: indicator.title,
    ...(indicator.description.trim() === '' ? {} : { description: indicator.description }),
    extension: [...own, ...extensions],
    scoring: { coding: [{ system: SCORING, code: scoringOf(indicator.calculation) }] },
    basis: indicator.basis,
    group: [{ ...group, population: writePopulations(indicator, populations(measure)), ...(stratifiers.length === 0 ? {} : { stratifier: stratifiers }) }, ...groups.slice(1)],
  }
}

/** A new indicator's Measure: a count of people, by the visit's start, of everyone with care. */
export function newIndicator(common: Json): Json {
  return writeIndicator(
    { resourceType: 'Measure', ...common },
    {
      title: typeof common['title'] === 'string' ? common['title'] : '',
      description: '',
      basis: 'Encounter',
      periodDate: 'Encounter.actualPeriod.start',
      counts: 'people',
      calculation: 'count',
      ratePer: 1000,
      cumulative: false,
      numerator: EVERYONE,
      denominator: null,
      value: { kind: 'expression', expression: '' },
      pick: 'first',
      disaggregations: [],
    },
  )
}

// ---------------------------------------------------------------------------------------------
// What the editor offers

/** A declared date an indicator may place care by or compare: what a person reads, and its
 * FHIRPath. */
export interface DeclaredDate {
  label: 'visitStart' | 'visitEnd' | 'observed' | 'enrollmentStart' | 'enrollmentEnd' | 'birthDate' | 'nextAppointment' | 'lastMenstrualPeriod'
  path: string
}

/** The declared dates of a record of `basis`, then the patient's. */
export function declaredDates(basis: string): DeclaredDate[] {
  const own: DeclaredDate[] =
    basis === 'Encounter'
      ? [
          { label: 'visitStart', path: 'Encounter.actualPeriod.start' },
          { label: 'visitEnd', path: 'Encounter.actualPeriod.end' },
        ]
      : basis === 'Observation'
        ? [{ label: 'observed', path: 'Observation.effective' }]
        : basis === 'EpisodeOfCare'
          ? [
              { label: 'enrollmentStart', path: 'EpisodeOfCare.period.start' },
              { label: 'enrollmentEnd', path: 'EpisodeOfCare.period.end' },
            ]
          : []
  return [
    ...own,
    { label: 'birthDate', path: '%patient.birthDate' },
    { label: 'nextAppointment', path: "%records.entry.resource.ofType(Appointment).where(status = 'booked').start.first()" },
    { label: 'lastMenstrualPeriod', path: "%records.entry.resource.ofType(Observation).where(code.coding.where(system = 'http://loinc.org' and code = '8665-2').exists()).value.first()" },
  ]
}

/** Inputs a simple condition may compare, as the tree reads them back. */
export const CONDITION_INPUTS = ['%patient.gender', '%resource.status', '%patient.deceased', '%patient.active'] as const

// ---------------------------------------------------------------------------------------------
// Problems

/** Another indicator of the package: `url|version`, and the ones it refers to. */
export interface Other {
  canonical: string
  title: string
  refers: string[]
}

/** What is wrong with the indicator, as Validation would refuse it. */
export type IndicatorProblem =
  | { kind: 'no-title' }
  | { kind: 'no-period-date' }
  | { kind: 'no-value' }
  | { kind: 'unpinned'; measure: string }
  | { kind: 'itself' }
  | { kind: 'unknown'; measure: string }
  | { kind: 'circle'; through: string[] }
  | { kind: 'no-disaggregation-name'; at: number }
  | { kind: 'twice'; name: string }

/** The other indicators `indicator` refers to, `url|version`, from both sides. */
export function referred(indicator: Indicator): string[] {
  const trees = [indicator.numerator, ...(indicator.denominator === null ? [] : [indicator.denominator])]
  return [...new Set(trees.flatMap(referencesOf).map((m) => m.measure))]
}

/** The indicator's problems. `self` is its own `url|version`; `others` the package's other
 * indicators. A circle is a path from the indicator back to itself through the others. */
export function indicatorProblems(indicator: Indicator, self: string, others: readonly Other[]): IndicatorProblem[] {
  const problems: IndicatorProblem[] = []
  if (indicator.title.trim() === '') problems.push({ kind: 'no-title' })
  if (indicator.periodDate.trim() === '') problems.push({ kind: 'no-period-date' })
  if (takesValue(indicator.calculation) && (indicator.value.kind === 'expression' ? indicator.value.expression.trim() === '' : indicator.value.from.trim() === '' || indicator.value.to.trim() === ''))
    problems.push({ kind: 'no-value' })
  const selfUrl = self.split('|')[0]
  for (const measure of referred(indicator)) {
    if (!measure.includes('|') || measure.endsWith('|')) problems.push({ kind: 'unpinned', measure })
    else if (measure === self || measure.split('|')[0] === selfUrl) problems.push({ kind: 'itself' })
    else if (!others.some((o) => o.canonical === measure)) problems.push({ kind: 'unknown', measure })
  }
  // Breadth first from what it refers to, back to itself.
  const byCanonical = new Map(others.map((o) => [o.canonical, o]))
  const queue: string[][] = referred(indicator)
    .filter((m) => byCanonical.has(m))
    .map((m) => [m])
  const seen = new Set<string>()
  while (queue.length > 0) {
    const path = queue.shift()!
    const last = path.at(-1)!
    if (seen.has(last)) continue
    seen.add(last)
    for (const next of byCanonical.get(last)?.refers ?? []) {
      if (next === self) {
        problems.push({ kind: 'circle', through: path })
        queue.length = 0
        break
      }
      if (byCanonical.has(next)) queue.push([...path, next])
    }
  }
  const names = new Set<string>()
  indicator.disaggregations.forEach((d, at) => {
    const name = d.kind === 'age' ? d.set : d.name
    if (d.kind === 'coded' && d.name.trim() === '') problems.push({ kind: 'no-disaggregation-name', at })
    else if (names.has(name)) problems.push({ kind: 'twice', name })
    names.add(name)
  })
  return problems
}

/** The other indicators a Measure's text refers to, from all its populations. */
export function refersOf(text: string): string[] {
  const read = readIndicator(text)
  return 'refusal' in read ? [] : referred(read.indicator)
}

// ---------------------------------------------------------------------------------------------
// Periods for *Try*

export const PERIOD_TYPES = ['weekly', 'monthly', 'quarterly', 'semiAnnual', 'annual', 'financialJuly'] as const
export type PeriodType = (typeof PERIOD_TYPES)[number]

/** The period of `type` holding `day` (`YYYY-MM-DD`), written as its type writes it. */
export function periodOf(type: PeriodType, day: string): string {
  const [year = 0, month = 1, date = 1] = day.split('-').map((n) => Number.parseInt(n, 10))
  switch (type) {
    case 'monthly':
      return `${year}-${String(month).padStart(2, '0')}`
    case 'quarterly':
      return `${year}Q${Math.ceil(month / 3)}`
    case 'semiAnnual':
      return `${year}S${month <= 6 ? 1 : 2}`
    case 'annual':
      return String(year)
    case 'financialJuly':
      return `${month >= 7 ? year : year - 1}July`
    case 'weekly': {
      // ISO week: the week with the year's first Thursday is week 1.
      const at = Date.UTC(year, month - 1, date)
      const weekday = (new Date(at).getUTCDay() + 6) % 7
      const thursday = new Date(at + (3 - weekday) * 86_400_000)
      const first = Date.UTC(thursday.getUTCFullYear(), 0, 1)
      const week = Math.floor((thursday.getTime() - first) / (7 * 86_400_000)) + 1
      return `${thursday.getUTCFullYear()}W${week}`
    }
  }
}

// ---------------------------------------------------------------------------------------------
// *Save as test*

/** The `type` of an indicator's saved *Try* in `tests/`, which submission's simulation runs
 * again (`health-publication`'s `indicators::run_tests`). */
export const INDICATOR_TEST_TYPE = 'indicator-test'

/** What Try gave, as a test keeps it: the count, the denominator, and the value as its exact
 * decimal text. */
export interface Tried {
  count: number
  denominator?: number | null | undefined
  value?: { toString: () => string } | null | undefined
}

/** The `tests/` file *Save as test* writes for indicator `measure` (`url|version`) titled
 * `title`, tried for `period` of `periodType`, and its path. */
export function indicatorTest(measure: string, title: string, periodType: PeriodType, period: string, tried: Tried, taken: readonly string[]): { path: string; test: Json } {
  const id = (measure.split('|')[0] ?? '').split('/').at(-1) ?? 'indicator'
  const base = `tests/indicator-${id}-${period}`.replace(/[^A-Za-z0-9/_-]+/g, '-')
  let path = `${base}.json`
  for (let n = 2; taken.includes(path); n += 1) path = `${base}-${n}.json`
  const expect: Json = { count: tried.count }
  if (tried.denominator !== null && tried.denominator !== undefined) expect['denominator'] = tried.denominator
  if (tried.value !== null && tried.value !== undefined) expect['value'] = tried.value.toString()
  return { path, test: { type: INDICATOR_TEST_TYPE, name: `${title}, ${period}`, measure, periodType, period, expect } }
}

// ---------------------------------------------------------------------------------------------
// Try's explanation (#864)

/** One condition of the numerator's or denominator's tree that *Try* marks held or not on each
 * record: every node under each side's root, depth first, a group as well as its conditions. */
export interface Explained {
  side: 'numerator' | 'denominator'
  /** 1 for a child of the side's root. */
  depth: number
  node: CriteriaNode
  /** The node as FHIRPath, as the side's criteria write it: what *Try* evaluates. */
  expression: string
}

/** The conditions *Try* explains, the numerator's first, then the denominator's. */
export function explainedConditions(indicator: Indicator): Explained[] {
  const out: Explained[] = []
  const walk = (side: Explained['side'], node: CriteriaNode, depth: number) => {
    out.push({ side, depth, node, expression: compileCriteria(node) })
    if ((node as { kind: string }).kind === 'group') for (const child of (node as Criteria).children) walk(side, child, depth + 1)
  }
  for (const child of indicator.numerator.children) walk('numerator', child, 1)
  for (const child of indicator.denominator?.children ?? []) walk('denominator', child, 1)
  return out
}
