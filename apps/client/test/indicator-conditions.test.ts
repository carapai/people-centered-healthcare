import { describe, expect, test } from 'vitest'

import { compileCriteria, parseCriteria, referencesOf, type Atom, type Criteria, type CriteriaNode, type DateCondition } from '../src/features/studio/indicators/conditions'

// An indicator's condition tree (#550): date conditions relative to the reporting period and
// membership in another indicator, saved as one FHIRPath expression and parsed back. The texts
// are the ones the server's tests evaluate (health-analytics-app `tests/conditions.rs`).

const D = 'Encounter.plannedEndDate'
/** A date condition without its kind and input. */
type Written = DateCondition extends infer C ? (C extends DateCondition ? Omit<C, 'kind' | 'input'> : never) : never
const date = (condition: Written, input = D): DateCondition => ({ kind: 'date', input, ...condition }) as DateCondition
const all = (...children: CriteriaNode[]): Criteria => ({ kind: 'group', join: 'all', children })

const NEXT_APPOINTMENT = "%records.entry.resource.ofType(Appointment).where(status = 'booked').start.first()"
const ANC_FIRST_VISITS = 'https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0'

const cases: [string, DateCondition, string][] = [
  ['is in the reporting period', date({ op: 'inPeriod' }), `(${D}.toDate() >= %periodStart.toDate() and ${D}.toDate() <= %periodEnd.toDate())`],
  ['is on the period start', date({ op: 'on', anchor: { at: 'periodStart' } }), `${D}.toDate() = %periodStart.toDate()`],
  ['is before the period start', date({ op: 'before', anchor: { at: 'periodStart' } }), `${D}.toDate() < %periodStart.toDate()`],
  ['is after the period end', date({ op: 'after', anchor: { at: 'periodEnd' } }), `${D}.toDate() > %periodEnd.toDate()`],
  ['is after a fixed date', date({ op: 'after', anchor: { at: 'date', date: '2026-03-15' } }), `${D}.toDate() > @2026-03-15`],
  [
    'is between a fixed date and the period end',
    date({ op: 'between', from: { at: 'date', date: '2026-03-10' }, to: { at: 'periodEnd' } }),
    `(${D}.toDate() >= @2026-03-10 and ${D}.toDate() <= %periodEnd.toDate())`,
  ],
  [
    'is within 4 weeks after the period end',
    date({ op: 'within', amount: 4, unit: 'weeks', direction: 'after', anchor: { at: 'periodEnd' } }),
    `(${D}.toDate() > %periodEnd.toDate() and ${D}.toDate() <= %periodEnd.toDate() + 4 weeks)`,
  ],
  [
    'is within 3 months before the period start',
    date({ op: 'within', amount: 3, unit: 'months', direction: 'before', anchor: { at: 'periodStart' } }),
    `(${D}.toDate() >= %periodStart.toDate() - 3 months and ${D}.toDate() < %periodStart.toDate())`,
  ],
  [
    'is within 10 days before the period end',
    date({ op: 'within', amount: 10, unit: 'days', direction: 'before', anchor: { at: 'periodEnd' } }),
    `(${D}.toDate() >= %periodEnd.toDate() - 10 days and ${D}.toDate() < %periodEnd.toDate())`,
  ],
  [
    'is within 1 month after a fixed date',
    date({ op: 'within', amount: 1, unit: 'months', direction: 'after', anchor: { at: 'date', date: '2026-01-31' } }),
    `(${D}.toDate() > @2026-01-31 and ${D}.toDate() <= @2026-01-31 + 1 month)`,
  ],
  ['is in the previous 3 periods', date({ op: 'previous', periods: 3 }), `(${D}.toDate() >= %previousPeriods.start[2].toDate() and ${D}.toDate() < %periodStart.toDate())`],
  ['is empty', date({ op: 'empty' }), `${D}.toDate().empty()`],
  ['is not empty', date({ op: 'notEmpty' }), `${D}.toDate().exists()`],
]

describe('a date condition', () => {
  test.each(cases)('%s compiles against the period and reads back', (_, condition, text) => {
    expect(compileCriteria(condition)).toBe(text)
    expect(parseCriteria(text)).toEqual(all(condition))
  })

  test('on any declared date, however its FHIRPath reads it', () => {
    const next = date({ op: 'within', amount: 4, unit: 'weeks', direction: 'after', anchor: { at: 'periodEnd' } }, NEXT_APPOINTMENT)
    const text = `(${NEXT_APPOINTMENT}.toDate() > %periodEnd.toDate() and ${NEXT_APPOINTMENT}.toDate() <= %periodEnd.toDate() + 4 weeks)`
    expect(compileCriteria(next)).toBe(text)
    expect(parseCriteria(text)).toEqual(all(next))
    // The reference package's ANC 1st visits with the next visit booked holds this numerator.
    expect(parseCriteria(text).children).toHaveLength(1)
  })

  test('sits in a tree with value conditions, groups and none of these', () => {
    const tree: Criteria = all(
      { kind: 'condition', input: '%patient.gender', op: '=', value: 'female' },
      { kind: 'group', join: 'not', children: [date({ op: 'empty' }, '%patient.birthDate'), date({ op: 'previous', periods: 2 }, 'Encounter.actualPeriod.start')] },
      { kind: 'group', join: 'any', children: [date({ op: 'inPeriod' }), date({ op: 'after', anchor: { at: 'periodEnd' } })] },
    )
    const text = compileCriteria(tree)
    expect(text).toBe(
      "%patient.gender = 'female' and (%patient.birthDate.toDate().empty() or (Encounter.actualPeriod.start.toDate() >= %previousPeriods.start[1].toDate() and Encounter.actualPeriod.start.toDate() < %periodStart.toDate())).not() and ((Encounter.plannedEndDate.toDate() >= %periodStart.toDate() and Encounter.plannedEndDate.toDate() <= %periodEnd.toDate()) or Encounter.plannedEndDate.toDate() > %periodEnd.toDate())",
    )
    expect(parseCriteria(text)).toEqual(tree)
  })

  test('text it did not write stays FHIRPath', () => {
    for (const text of [`${D}.toDate() <= %periodEnd.toDate() + 4 weeks`, `(${D}.toDate() >= @2026-01-01 and Encounter.other.toDate() <= @2026-02-01)`, `${D} > %periodEnd`]) {
      expect(parseCriteria(text)).toEqual(all({ kind: 'fhirpath', expression: text }))
    }
  })
})

describe('in another indicator', () => {
  const numerator: Atom = { kind: 'member', measure: ANC_FIRST_VISITS, side: 'numerator' }

  test('is pinned to url|version and reads back', () => {
    const text = `'${ANC_FIRST_VISITS}' in %indicators.numerator`
    expect(compileCriteria(numerator)).toBe(text)
    expect(parseCriteria(text)).toEqual(all(numerator))
  })

  test('sits in a tree, and the indicators it refers to are listed', () => {
    const denominator: Atom = { kind: 'member', measure: 'https://health.go.ug/fhir/Measure/anc-visits|1.0.0', side: 'denominator' }
    const tree = all(numerator, { kind: 'group', join: 'not', children: [denominator] })
    const text = compileCriteria(tree)
    expect(text).toBe(`'${ANC_FIRST_VISITS}' in %indicators.numerator and ('https://health.go.ug/fhir/Measure/anc-visits|1.0.0' in %indicators.denominator).not()`)
    expect(parseCriteria(text)).toEqual(tree)
    expect(referencesOf(tree)).toEqual([numerator, denominator])
  })
})
