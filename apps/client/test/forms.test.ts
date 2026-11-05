import type { Questionnaire, QuestionnaireResponse, QuestionnaireResponseItem } from '@health/fhir-types'
import { LosslessNumber, stringify } from 'lossless-json'
import { describe, expect, test } from 'vitest'

import {
  CALCULATED_EXPRESSION,
  CONSTRAINT,
  ENABLE_WHEN_EXPRESSION,
  addRepeat,
  calculated,
  initialResponse,
  isEnabled,
  itemsByLinkId,
  missing,
  removeRepeat,
  responseItems,
  settled,
  violations,
  withAnswers,
} from '../src/features/forms/model'

const qr = 'QuestionnaireResponse.item'
const n = (text: string) => new LosslessNumber(text)

const anc: Questionnaire = {
  resourceType: 'Questionnaire',
  url: 'https://health.go.ug/fhir/Questionnaire/anc-visit',
  version: '2.0.0',
  status: 'active',
  item: [
    {
      linkId: 'age',
      text: 'Age',
      type: 'integer',
      required: true,
      extension: [
        {
          url: CONSTRAINT,
          extension: [
            { url: 'key', valueId: 'age-plausible' },
            { url: 'severity', valueCode: 'error' },
            { url: 'expression', valueString: `${qr}.where(linkId = 'age').answer.value.empty() or ${qr}.where(linkId = 'age').answer.value >= 10` },
            { url: 'human', valueString: 'A pregnant client is at least 10 years old' },
          ],
        },
      ],
    },
    { linkId: 'pregnant', text: 'Pregnant?', type: 'boolean' },
    { linkId: 'lmp', text: 'Last menstrual period', type: 'date', required: true, enableWhen: [{ question: 'pregnant', operator: '=', answerBoolean: true }] },
    {
      linkId: 'adolescent-counselling',
      text: 'Adolescent counselling given',
      type: 'boolean',
      extension: [{ url: ENABLE_WHEN_EXPRESSION, valueExpression: { language: 'text/fhirpath', expression: `%resource.item.where(linkId = 'age').answer.value < 18` } }],
    },
    { linkId: 'weight-start', text: 'Weight at booking (kg)', type: 'decimal' },
    { linkId: 'weight-gain', text: 'Weight gained (kg)', type: 'decimal' },
    {
      linkId: 'weight-now',
      text: 'Weight now (kg)',
      type: 'decimal',
      readOnly: true,
      extension: [
        {
          url: CALCULATED_EXPRESSION,
          valueExpression: { language: 'text/fhirpath', expression: `%resource.item.where(linkId = 'weight-start').answer.value + %resource.item.where(linkId = 'weight-gain').answer.value` },
        },
      ],
    },
    { linkId: 'child', text: 'Previous children', type: 'group', repeats: true, item: [{ linkId: 'child-year', text: 'Year born', type: 'integer' }] },
    { linkId: 'note', text: 'Read this first', type: 'display' },
  ],
} as unknown as Questionnaire

const id = (response: QuestionnaireResponse, linkId: string) => responseItems(response.item).find((i) => i.linkId === linkId)?.id ?? ''
const answer = (response: QuestionnaireResponse, linkId: string, value: object) => withAnswers(response, id(response, linkId), [value as never])
const item = (linkId: string) => itemsByLinkId(anc).get(linkId)!

describe('the questionnaire renderer model', () => {
  test('a new response has an item, with a stable id, for each question but display text', () => {
    const response = initialResponse(anc)
    expect(response.questionnaire).toBe('https://health.go.ug/fhir/Questionnaire/anc-visit|2.0.0')
    expect(response.status).toBe('in-progress')
    expect(response.item?.map((i) => i.linkId)).toEqual(['age', 'pregnant', 'lmp', 'adolescent-counselling', 'weight-start', 'weight-gain', 'weight-now', 'child'])
    const ids = responseItems(response.item).map((i) => i.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(response.item?.[7]?.item?.map((i) => i.linkId)).toEqual(['child-year'])
  })

  test('enableWhen follows the answers it names', () => {
    let response = initialResponse(anc)
    expect(isEnabled(item('lmp'), response)).toBe(false)
    response = answer(response, 'pregnant', { valueBoolean: true })
    expect(isEnabled(item('lmp'), response)).toBe(true)
    response = answer(response, 'pregnant', { valueBoolean: false })
    expect(isEnabled(item('lmp'), response)).toBe(false)
  })

  test('enableWhenExpression is evaluated with fhirpath.js', () => {
    let response = initialResponse(anc)
    expect(isEnabled(item('adolescent-counselling'), response)).toBe(false)
    response = answer(response, 'age', { valueInteger: n('16') })
    expect(isEnabled(item('adolescent-counselling'), response)).toBe(true)
    response = answer(response, 'age', { valueInteger: n('25') })
    expect(isEnabled(item('adolescent-counselling'), response)).toBe(false)
  })

  // fhirpath.js keeps the value exact but not the scale of an arithmetic result (61.6, where the
  // server gives 61.60): a preview, recomputed by the server on submit (ADR 0052).
  test('a calculated value is exact, and settled into the response', () => {
    let response = initialResponse(anc)
    expect(calculated(item('weight-now'), response)).toBeUndefined()
    response = answer(response, 'weight-start', { valueDecimal: n('61.50') })
    response = answer(response, 'weight-gain', { valueDecimal: n('0.10') })
    expect(stringify(calculated(item('weight-now'), response))).toBe('{"valueDecimal":61.6}')
    const saved = settled(anc, response)
    const now = responseItems(saved.item).find((i) => i.linkId === 'weight-now')
    expect(stringify(now?.answer)).toBe('[{"valueDecimal":61.6}]')
  })

  test('constraints and required questions are reported', () => {
    let response = initialResponse(anc)
    expect(missing(anc, response)).toEqual(['age'])
    expect(violations(anc, response)).toEqual([])
    response = answer(response, 'age', { valueInteger: n('8') })
    expect(violations(anc, response)).toEqual([
      { linkId: 'age', key: 'age-plausible', severity: 'error', human: 'A pregnant client is at least 10 years old' },
    ])
    response = answer(response, 'pregnant', { valueBoolean: true })
    expect(missing(anc, response)).toEqual(['lmp'])
  })

  test('disabled questions keep no answers once settled', () => {
    let response = initialResponse(anc)
    response = answer(response, 'pregnant', { valueBoolean: true })
    response = answer(response, 'lmp', { valueDate: '2026-07-01' })
    response = answer(response, 'pregnant', { valueBoolean: false })
    const saved = settled(anc, response)
    expect(responseItems(saved.item).some((i) => i.linkId === 'lmp')).toBe(false)
  })

  test('repeats get their own stable ids, and the last one stays', () => {
    let response = initialResponse(anc)
    const first = id(response, 'child')
    response = addRepeat(anc, response, 'child')
    const children = (response.item ?? []).filter((i: QuestionnaireResponseItem) => i.linkId === 'child')
    expect(children).toHaveLength(2)
    expect(children[0]?.id).toBe(first)
    expect(children[1]?.id).not.toBe(first)
    response = removeRepeat(response, first)
    expect((response.item ?? []).filter((i) => i.linkId === 'child').map((i) => i.id)).toEqual([children[1]?.id])
    response = removeRepeat(response, children[1]?.id ?? '')
    expect((response.item ?? []).filter((i) => i.linkId === 'child')).toHaveLength(1)
    expect(addRepeat(anc, response, 'age')).toBe(response)
  })
})
