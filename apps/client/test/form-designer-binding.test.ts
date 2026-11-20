import type { Questionnaire, QuestionnaireItem } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import { choices } from '../src/features/forms/model'
import { boundValueSets, layoutWidget, problems, splitBinding, withType, withValueSet } from '../src/features/studio/forms/model'

// #497 part 3: a choice question bound to a ValueSet through the Terminology picker. The binding
// records `url|version`; the preview offers the ValueSet's codes.

const triage = 'https://health.go.ug/fhir/ValueSet/triage-all'
const choice: QuestionnaireItem = {
  linkId: 'category',
  text: 'Category',
  type: 'coding',
  answerOption: [{ valueCoding: { code: 'a', display: 'A' } }],
}

describe('binding a ValueSet', () => {
  test('binding records url|version and replaces the item’s own options', () => {
    const bound = withValueSet(choice, `${triage}|1.0.0`)
    expect(bound.answerValueSet).toBe(`${triage}|1.0.0`)
    expect(bound.answerOption).toBeUndefined()
    expect(bound.text).toBe('Category')
  })

  test('unbinding leaves the item with neither, so it is listed as a choice with no options', () => {
    const unbound = withValueSet(withValueSet(choice, triage), null)
    expect(unbound.answerValueSet).toBeUndefined()
    const form: Questionnaire = { resourceType: 'Questionnaire', status: 'draft', item: [unbound] }
    expect(problems(form, undefined, new Set())).toContainEqual({ code: 'no-options', path: [0], linkId: 'category' })
    const bound: Questionnaire = { ...form, item: [withValueSet(choice, triage)] }
    expect(problems(bound, undefined, new Set())).toEqual([])
  })

  test('a question that is no longer a choice drops its binding', () => {
    expect(withType(withValueSet(choice, triage), 'string').answerValueSet).toBeUndefined()
  })

  test('a binding splits into its url and version', () => {
    expect(splitBinding(`${triage}|1.0.0`)).toEqual({ url: triage, version: '1.0.0' })
    expect(splitBinding(triage)).toEqual({ url: triage, version: null })
  })

  test('the ValueSets a form binds, each once, nested questions too', () => {
    const form: Questionnaire = {
      resourceType: 'Questionnaire',
      status: 'draft',
      item: [withValueSet(choice, triage), { linkId: 'g', type: 'group', item: [{ ...withValueSet(choice, triage), linkId: 'again' }, { ...withValueSet(choice, 'urn:other|2'), linkId: 'other' }] }, choice],
    }
    expect(boundValueSets(form)).toEqual([triage, 'urn:other|2'])
  })

  test('the suggested widget as a Form Layout names it: search-as-you-type shows as a drop-down', () => {
    expect([3, 7, 8, 30, 31].map(layoutWidget)).toEqual(['radio', 'radio', 'select', 'select', 'select'])
  })
})

describe('the renderer’s choices', () => {
  test('a bound question offers the codes it is given for its ValueSet; its own options otherwise', () => {
    const codes = [{ system: 'urn:s', code: 'x', display: 'X' }]
    expect(choices(withValueSet(choice, triage), { [triage]: codes })).toEqual(codes)
    expect(choices(withValueSet(choice, triage), {})).toEqual([])
    expect(choices(withValueSet(choice, triage))).toEqual([])
    expect(choices(choice, { [triage]: codes })).toEqual([{ code: 'a', display: 'A' }])
  })
})
