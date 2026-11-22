import { describe, expect, test } from 'vitest'

import firstVisitBefore12Weeks from '../../../packages/reference-packages/anc/Measure-anc-first-visit-before-12-weeks.json'
import firstVisits from '../../../packages/reference-packages/anc/Measure-anc-first-visits.json'
import gestationalAge from '../../../packages/reference-packages/anc/Measure-anc-gestational-age-at-first-visit.json'
import { editorOf, newDesign, sections } from '../src/features/studio/designs'
import type { Criteria } from '../src/features/studio/indicators/conditions'
import { addDenominator, indicatorProblems, indicatorTest, newIndicator, periodOf, readIndicator, removeDenominator, withCalculation, writeIndicator, type Indicator } from '../src/features/studio/indicators/indicator'

// The indicator editor (#537): a Measure read as what the editor edits and written back, the
// denominator added and removed, the problems Validation would refuse, and Try's periods.

const GA = "%records.entry.resource.ofType(Observation).where(encounter.reference = 'Encounter/' + %resource.id and code.coding.where(system = 'http://loinc.org' and code = '18185-9').exists()).value.value"
const ANC_FIRST_VISITS = 'https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0'
const SELF = 'https://health.go.ug/fhir/Measure/anc-first-visit-before-12-weeks|1.0.0'

/** The texts the editor writes for *ANC 1st visit before 12 weeks* built as the issue says; the
 * server's test (`central-server` `studio::tests`) evaluates exactly these. */
const BUILT_NUMERATOR = `${GA} < 12`
const BUILT_DENOMINATOR = `'${ANC_FIRST_VISITS}' in %indicators.numerator and ((%patient.birthDate.toDate() >= %periodEnd.toDate() - 120 months and %patient.birthDate.toDate() < %periodEnd.toDate())).not()`

const read = (value: unknown) => {
  const result = readIndicator(JSON.stringify(value))
  if ('refusal' in result) throw new Error(result.refusal)
  return result
}

/** The Measure's first group's populations, by code, as `[code, expression]`. */
const populationsOf = (measure: Record<string, unknown>) =>
  ((measure['group'] as { population: { code: { coding: { code: string }[] }; criteria?: { expression: string } }[] }[])[0]?.population ?? []).map((p) => [p.code.coding[0]?.code, p.criteria?.expression])

describe('ANC 1st visit before 12 weeks, built in the editor', () => {
  // Start from a new count, as *New indicator* makes it.
  const start = read(newIndicator({ id: 'anc-first-visit-before-12-weeks', url: SELF.split('|')[0], version: '1.0.0', name: 'AncFirstVisitBefore12Weeks', title: 'ANC 1st visit before 12 weeks', status: 'draft' }))
  const numerator: Criteria = { kind: 'group', join: 'all', children: [{ kind: 'fhirpath', expression: `${GA} < 12` }] }
  const denominator: Criteria = {
    kind: 'group',
    join: 'all',
    children: [
      { kind: 'member', measure: ANC_FIRST_VISITS, side: 'numerator' },
      // None of these: age < 10, a birth date within the 120 months before the period's end.
      { kind: 'group', join: 'not', children: [{ kind: 'date', input: '%patient.birthDate', op: 'within', amount: 120, unit: 'months', direction: 'before', anchor: { at: 'periodEnd' } }] },
    ],
  }
  const built: Indicator = { ...addDenominator(start.indicator), numerator, denominator }

  test('the numerator over the denominator saves as a proportion with each side compiled', () => {
    expect(built.calculation).toBe('percentage')
    const measure = writeIndicator(start.measure, built)
    expect(measure['scoring']).toEqual({ coding: [{ system: 'http://terminology.hl7.org/CodeSystem/measure-scoring', code: 'proportion' }] })
    expect(populationsOf(measure)).toEqual([
      ['initial-population', 'true'],
      ['denominator', BUILT_DENOMINATOR],
      ['numerator', BUILT_NUMERATOR],
    ])
    // It reads back as built.
    expect(read(measure).indicator).toEqual(built)
    // And it has what Validation needs.
    expect(indicatorProblems(read(measure).indicator, SELF, [{ canonical: ANC_FIRST_VISITS, title: 'ANC 1st visits', refers: [] }])).toEqual([])
  })

  test('a reference indicator saved unchanged keeps its meaning', () => {
    for (const reference of [firstVisitBefore12Weeks, firstVisits, gestationalAge]) {
      const { measure, indicator } = read(reference)
      const written = writeIndicator(measure, indicator)
      // Only the populations' ids are added where they had none.
      expect(JSON.parse(JSON.stringify(written).replaceAll('"id":"initial-population",', ''))).toEqual(JSON.parse(JSON.stringify(reference).replaceAll('"id":"initial-population",', '')))
    }
    const median = read(gestationalAge).indicator
    expect(median).toMatchObject({ calculation: 'median', counts: 'people', pick: 'first', periodDate: 'Encounter.actualPeriod.start', denominator: null })
    expect(read(firstVisits).indicator.disaggregations).toEqual([{ kind: 'age', set: 'https://health.go.ug/fhir/Library/age-bands-anc|1.0.0' }])
  })
})

test('removing the denominator turns the indicator into a count, saved as cohort scoring', () => {
  const { measure, indicator } = read(firstVisitBefore12Weeks)
  expect(indicator.calculation).toBe('percentage')
  const count = removeDenominator(indicator)
  expect(count).toMatchObject({ calculation: 'count', denominator: null })
  const written = writeIndicator(measure, count)
  expect(written['scoring']).toEqual({ coding: [{ system: 'http://terminology.hl7.org/CodeSystem/measure-scoring', code: 'cohort' }] })
  // The numerator's conditions are what it counts.
  expect(populationsOf(written)).toEqual([['initial-population', BUILT_NUMERATOR]])
  // Adding it back is a percentage of everyone with care, to be narrowed.
  const again = addDenominator(count)
  expect(again).toMatchObject({ calculation: 'percentage', denominator: { kind: 'group', join: 'all', children: [] } })
  // A rate keeps its factor; a value calculation has no denominator.
  expect(writeIndicator(measure, { ...withCalculation(indicator, 'rate'), ratePer: 10000 })['extension']).toContainEqual({ url: 'https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-rate-per', valueInteger: 10000 })
  expect(withCalculation(indicator, 'sum').denominator).toBeNull()
})

const member = (measure: string): Criteria => ({ kind: 'group', join: 'all', children: [{ kind: 'member', measure, side: 'numerator' }] })

describe('problems', () => {
  const { indicator } = read(firstVisitBefore12Weeks)

  test('two indicators that refer to each other are a circle', () => {
    const other = { canonical: ANC_FIRST_VISITS, title: 'ANC 1st visits', refers: [SELF] }
    const problems = indicatorProblems({ ...indicator, denominator: member(ANC_FIRST_VISITS) }, SELF, [other])
    expect(problems).toEqual([{ kind: 'circle', through: [ANC_FIRST_VISITS] }])
    // Through a third one too.
    const third = 'https://health.go.ug/fhir/Measure/third|1.0.0'
    expect(indicatorProblems({ ...indicator, denominator: member(ANC_FIRST_VISITS) }, SELF, [{ ...other, refers: [third] }, { canonical: third, title: 'Third', refers: [SELF] }])).toEqual([{ kind: 'circle', through: [ANC_FIRST_VISITS, third] }])
    // Not when the other does not refer back.
    expect(indicatorProblems({ ...indicator, denominator: member(ANC_FIRST_VISITS) }, SELF, [{ ...other, refers: [] }])).toEqual([])
  })

  test('itself, an unpinned or unknown indicator, no name, no period date, a disaggregation twice', () => {
    const problems = indicatorProblems(
      {
        ...indicator,
        title: ' ',
        periodDate: '',
        numerator: { kind: 'group', join: 'any', children: [{ kind: 'member', measure: SELF, side: 'denominator' }, { kind: 'member', measure: 'https://x/Measure/y', side: 'numerator' }, { kind: 'member', measure: 'https://x/Measure/z|2.0.0', side: 'numerator' }] },
        disaggregations: [
          { kind: 'coded', name: 'sex', criteria: '%patient.gender' },
          { kind: 'coded', name: 'sex', criteria: '%patient.gender' },
          { kind: 'coded', name: '', criteria: '' },
        ],
      },
      SELF,
      [],
    )
    expect(problems.map((p) => p.kind)).toEqual(['no-title', 'no-period-date', 'itself', 'unpinned', 'unknown', 'twice', 'no-disaggregation-name'])
  })
})

test('a disaggregation by a coded value is a stratifier with its criteria, named by its code', () => {
  const { measure, indicator } = read(firstVisits)
  const written = writeIndicator(measure, { ...indicator, disaggregations: [...indicator.disaggregations, { kind: 'coded', name: 'sex', criteria: '%patient.gender' }] })
  const stratifiers = (written['group'] as { stratifier: unknown[] }[])[0]?.stratifier
  expect(stratifiers).toEqual([
    {
      extension: [{ url: 'https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-age-band-set', valueCanonical: 'https://health.go.ug/fhir/Library/age-bands-anc|1.0.0' }],
      description: "By MoH's ANC age bands: the woman's age in whole years at the end of the period",
    },
    { code: { coding: [{ code: 'sex' }] }, criteria: { language: 'text/fhirpath', expression: '%patient.gender' } },
  ])
  expect(read(written).indicator.disaggregations[1]).toEqual({ kind: 'coded', name: 'sex', criteria: '%patient.gender' })
})

test('Try names the period of each type that holds a day as the server reads it', () => {
  expect(periodOf('monthly', '2026-03-04')).toBe('2026-03')
  expect(periodOf('quarterly', '2026-03-04')).toBe('2026Q1')
  expect(periodOf('semiAnnual', '2026-07-01')).toBe('2026S2')
  expect(periodOf('annual', '2026-07-01')).toBe('2026')
  expect(periodOf('financialJuly', '2026-03-04')).toBe('2025July')
  expect(periodOf('financialJuly', '2026-07-04')).toBe('2026July')
  expect(periodOf('weekly', '2026-03-04')).toBe('2026W10')
  expect(periodOf('weekly', '2027-01-01')).toBe('2026W53')
})

test('Indicators is a section of the Studio, its New a count that opens in the indicator editor', () => {
  expect(sections.find((s) => s.id === 'indicators')).toEqual({ id: 'indicators', kinds: ['indicator'], creates: true })
  const made = newDesign('indicator', 'ANC visits', 'https://health.go.ug/fhir', '1.1.0-draft', [])
  expect(made.path).toBe('Measure-anc-visits.json')
  expect(made.resource).toMatchObject({ resourceType: 'Measure', url: 'https://health.go.ug/fhir/Measure/anc-visits', version: '1.1.0-draft', title: 'ANC visits', basis: 'Encounter', scoring: { coding: [{ code: 'cohort' }] } })
  expect(read(made.resource).indicator).toMatchObject({ calculation: 'count', counts: 'people', periodDate: 'Encounter.actualPeriod.start' })
  expect(editorOf({ kind: 'indicator', path: made.path, table: null }, 'ug.moh.anc@1.1.0-draft')).toEqual({ to: '/studio/$id/indicators/$', params: { id: 'ug.moh.anc@1.1.0-draft', _splat: made.path } })
})

test('Save as test keeps what Try gave for the saved indicator, as submission runs it again', () => {
  const made = indicatorTest(SELF, 'ANC 1st visit before 12 weeks', 'monthly', '2026-03', { count: 3, denominator: 4, value: { toString: () => '75' } }, [])
  expect(made).toEqual({
    path: 'tests/indicator-anc-first-visit-before-12-weeks-2026-03.json',
    test: { type: 'indicator-test', name: 'ANC 1st visit before 12 weeks, 2026-03', measure: SELF, periodType: 'monthly', period: '2026-03', expect: { count: 3, denominator: 4, value: '75' } },
  })
  // A count keeps no denominator; a path already taken gets a number.
  const again = indicatorTest(SELF, 'x', 'quarterly', '2026Q1', { count: 7, denominator: null, value: { toString: () => '7' } }, ['tests/indicator-anc-first-visit-before-12-weeks-2026Q1.json'])
  expect(again.path).toBe('tests/indicator-anc-first-visit-before-12-weeks-2026Q1-2.json')
  expect(again.test['expect']).toEqual({ count: 7, value: '7' })
})
