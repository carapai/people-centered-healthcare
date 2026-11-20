import type { Questionnaire } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import { FORM_LAYOUT_SCHEMA, type FormLayout } from '../src/features/forms/layout'
import { blocks, pruneLayout, readQuestionnaire, replaceWith } from '../src/features/studio/forms/importing'

// Import FHIR JSON in the form designer (#497): a pasted Questionnaire replaces the form; R4
// content is converted with a conversion report, and anything unconverted blocks the import.

const draft: Questionnaire = { resourceType: 'Questionnaire', id: 'opd-triage', url: 'https://health.go.ug/fhir/Questionnaire/opd-triage', version: '1.1.0-draft', status: 'draft', item: [{ linkId: 'old', text: 'Old', type: 'string' }] }

const r4 = {
  resourceType: 'Questionnaire',
  url: 'http://smart.who.int/anc/Questionnaire/ANCDT01',
  version: '1.0.0',
  status: 'active',
  title: 'ANC contact',
  item: [
    {
      linkId: 'visit',
      type: 'group',
      text: 'Visit',
      item: [
        { linkId: 'danger', type: 'choice', text: 'Danger signs', answerOption: [{ valueCoding: { code: 'bleeding', display: 'Bleeding' } }] },
        { linkId: 'other', type: 'open-choice', text: 'Other', answerValueSet: 'http://example.org/ValueSet/x' },
      ],
    },
  ],
}

describe('reading a pasted Questionnaire', () => {
  test('R4 choice and open-choice become coding with answerConstraint, each in the report', () => {
    const read = readQuestionnaire(JSON.stringify(r4), 'R4')
    expect(read.ok).toBe(true)
    if (!read.ok) return
    const [danger, other] = read.questionnaire.item?.[0]?.item ?? []
    expect(danger).toMatchObject({ type: 'coding', answerConstraint: 'optionsOnly' })
    expect(other).toMatchObject({ type: 'coding', answerConstraint: 'optionsOrString' })
    expect(read.report).toEqual([
      { outcome: 'converted', code: 'choice', at: 'danger' },
      { outcome: 'converted', code: 'open-choice', at: 'other' },
    ])
    expect(blocks(read.report)).toBe(false)
  })

  test('an R5 paste holding R4 choice types is refused as looking like R4', () => {
    expect(readQuestionnaire(JSON.stringify(r4), 'R5')).toEqual({ ok: false, refusal: 'looks-r4' })
  })

  test('what cannot be converted is reported and blocks the import', () => {
    const odd = { ...r4, contained: [{ resourceType: 'Observation', id: 't' }], somethingElse: 1, item: [{ linkId: 'a', type: 'question', text: 'A', mystery: true, modifierExtension: [{ url: 'x' }] }] }
    const read = readQuestionnaire(JSON.stringify(odd), 'R4')
    expect(read.ok).toBe(true)
    if (!read.ok) return
    expect(read.report).toEqual(
      expect.arrayContaining([
        { outcome: 'unconverted', code: 'unknown-element', at: 'Questionnaire', element: 'somethingElse' },
        { outcome: 'unconverted', code: 'contained', at: 'Questionnaire' },
        { outcome: 'unconverted', code: 'unknown-element', at: 'a', element: 'mystery' },
        { outcome: 'unconverted', code: 'modifier-extension', at: 'a' },
        { outcome: 'unconverted', code: 'unknown-type', at: 'a', type: 'question' },
      ]),
    )
    expect(blocks(read.report)).toBe(true)
  })

  test('primitive extensions (_text) are known elements, and an R5 paste converts nothing', () => {
    const r5 = { resourceType: 'Questionnaire', status: 'active', _title: { extension: [] }, item: [{ linkId: 'a', type: 'coding', _text: { extension: [] }, answerOption: [] }] }
    const read = readQuestionnaire(JSON.stringify(r5), 'R5')
    expect(read).toMatchObject({ ok: true, report: [] })
  })

  test('not JSON, or not a Questionnaire, is refused', () => {
    expect(readQuestionnaire('{', 'R5')).toEqual({ ok: false, refusal: 'not-json' })
    expect(readQuestionnaire('{"resourceType":"Patient"}', 'R5')).toEqual({ ok: false, refusal: 'not-questionnaire' })
  })
})

describe('replacing the form', () => {
  test('the draft keeps its id, url, version and status; the source is kept in derivedFrom', () => {
    const read = readQuestionnaire(JSON.stringify(r4), 'R4')
    if (!read.ok) throw new Error('not read')
    const replaced = replaceWith(draft, read.questionnaire, read.report)
    expect(replaced.questionnaire).toMatchObject({ id: 'opd-triage', url: draft.url, version: '1.1.0-draft', status: 'draft', title: 'ANC contact', derivedFrom: ['http://smart.who.int/anc/Questionnaire/ANCDT01|1.0.0'] })
    expect(replaced.questionnaire.item?.[0]?.linkId).toBe('visit')
    expect(replaced.report.at(-1)).toEqual({ outcome: 'converted', code: 'source', at: 'Questionnaire', source: 'http://smart.who.int/anc/Questionnaire/ANCDT01|1.0.0' })
  })

  test('re-importing the draft’s own Questionnaire records no source', () => {
    const replaced = replaceWith(draft, { ...draft, title: 'Again' }, [])
    expect(replaced.questionnaire.derivedFrom).toBeUndefined()
    expect(replaced.report).toEqual([])
  })

  test('the layout keeps only the questions the new form has', () => {
    const layout: FormLayout = { $schema: FORM_LAYOUT_SCHEMA, url: 'u', version: '1', questionnaire: 'q|1', sections: [{ title: 'S', items: ['old', 'visit'] }], items: { old: { hint: 'h' }, danger: { widget: 'radio' } } }
    const next = { ...draft, item: [{ linkId: 'visit', type: 'group', item: [{ linkId: 'danger', type: 'coding' }] }] }
    expect(pruneLayout(layout, next)).toEqual({ ...layout, sections: [{ title: 'S', items: ['visit'] }], items: { danger: { widget: 'radio' } } })
  })
})
