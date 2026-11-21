import type { Questionnaire } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import { FORM_LAYOUT_SCHEMA, type FormLayout } from '../src/features/forms/layout'
import { importQuestionnaire } from '../src/features/studio/api'
import { importResult, pruneLayout, replaceWith } from '../src/features/studio/forms/importing'
import { fakeTransport } from './fake'

// Import FHIR JSON in the form designer (#497, #853): central reads the pasted Questionnaire,
// converting R4 to R5 with health-fhir-mapping; the report and whether it blocks are the server's.

const draft: Questionnaire = { resourceType: 'Questionnaire', id: 'opd-triage', url: 'https://health.go.ug/fhir/Questionnaire/opd-triage', version: '1.1.0-draft', status: 'draft', item: [{ linkId: 'old', text: 'Old', type: 'string' }] }

// What central answers for an R4 ANC contact, converted.
const converted = {
  resourceType: 'Questionnaire',
  url: 'http://smart.who.int/anc/Questionnaire/ANCDT01',
  version: '1.0.0',
  status: 'active',
  title: 'ANC contact',
  item: [{ linkId: 'visit', type: 'group', text: 'Visit', item: [{ linkId: 'danger', type: 'coding', answerConstraint: 'optionsOnly', text: 'Danger signs' }] }],
}
const report = [{ outcome: 'converted', code: 'choice-to-coding', at: 'danger', element: null, type: null }] as const
const answer = { refusal: null, questionnaire: converted, report: [...report], blocked: false }

describe('reading a pasted Questionnaire on central', () => {
  test('the paste goes to central with its version; the converted form and report come back whole', async () => {
    const transport = fakeTransport(() => ({ body: answer }))
    const read = importResult(await importQuestionnaire(transport, 'ug.moh.anc@1.1.0-draft', '{"resourceType":"Questionnaire"}', 'R4'))
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/studio/packages/ug.moh.anc%401.1.0-draft/questionnaires/import', body: { text: '{"resourceType":"Questionnaire"}', version: 'R4' } })
    // Every member of the resource survives the response schema.
    expect(read).toEqual({ ok: true, questionnaire: converted, report, blocked: false })
  })

  test('what central could not convert blocks the import', async () => {
    const unconverted = { outcome: 'unconverted', code: 'unknown-type', at: 'a', element: null, type: 'question' }
    const transport = fakeTransport(() => ({ body: { ...answer, report: [unconverted], blocked: true } }))
    expect(importResult(await importQuestionnaire(transport, 'x@1.0.0', '{}', 'R4'))).toMatchObject({ ok: true, blocked: true, report: [unconverted] })
  })

  test('central’s refusals are passed on', async () => {
    for (const refusal of ['not-json', 'not-questionnaire', 'looks-r4'] as const) {
      const transport = fakeTransport(() => ({ body: { refusal, questionnaire: null, report: [], blocked: true } }))
      expect(importResult(await importQuestionnaire(transport, 'x@1.0.0', '{', 'R5'))).toEqual({ ok: false, refusal })
    }
  })
})

describe('replacing the form', () => {
  test('the draft keeps its id, url, version and status; the source is kept in derivedFrom', () => {
    const replaced = replaceWith(draft, converted as unknown as Questionnaire, [...report])
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
