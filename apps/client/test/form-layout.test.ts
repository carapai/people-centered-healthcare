import type { Questionnaire } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import { FORM_LAYOUT_SCHEMA, type FormLayout, sections, widget } from '../src/features/forms/layout'

const questionnaire = {
  resourceType: 'Questionnaire',
  url: 'https://health.go.ug/fhir/Questionnaire/anc-visit',
  version: '2.0.0',
  status: 'active',
  item: [
    { linkId: 'age', type: 'integer' },
    { linkId: 'pregnant', type: 'boolean' },
    { linkId: 'danger-sign', type: 'coding' },
    { linkId: 'note', type: 'text' },
  ],
} as unknown as Questionnaire

const layout: FormLayout = {
  $schema: FORM_LAYOUT_SCHEMA,
  url: 'https://health.go.ug/fhir/layout/anc-visit',
  version: '1.0.0',
  questionnaire: 'https://health.go.ug/fhir/Questionnaire/anc-visit|2.0.0',
  sections: [
    { title: 'Danger signs first', items: ['danger-sign', 'missing'] },
    { title: 'History', items: ['age', 'danger-sign'] },
  ],
  items: { pregnant: { widget: 'checkbox' }, 'danger-sign': { widget: 'radio' }, age: { widget: 'textarea' } },
}

const ids = (chosen?: FormLayout) => sections(questionnaire, chosen).map((s) => [s.title, s.items.map((i) => i.linkId)])

function item(linkId: string) {
  const found = questionnaire.item?.find((i) => i.linkId === linkId)
  if (found === undefined) throw new Error(`no item ${linkId}`)
  return found
}

describe('Form Layouts', () => {
  test('sections follow the layout, and items it does not place are still shown after them', () => {
    expect(ids(layout)).toEqual([
      ['Danger signs first', ['danger-sign']],
      ['History', ['age']],
      [undefined, ['pregnant', 'note']],
    ])
  })

  test('a layout for another Questionnaire version is not applied', () => {
    expect(ids({ ...layout, questionnaire: 'https://health.go.ug/fhir/Questionnaire/anc-visit|1.0.0' })).toEqual([[undefined, ['age', 'pregnant', 'danger-sign', 'note']]])
    expect(ids(undefined)).toEqual([[undefined, ['age', 'pregnant', 'danger-sign', 'note']]])
  })

  test('a widget is used only where it fits the question', () => {
    expect(widget(item('pregnant'), layout)).toBe('checkbox')
    expect(widget(item('danger-sign'), layout)).toBe('radio')
    expect(widget(item('age'), layout)).toBe('input')
    expect(widget(item('note'), undefined)).toBe('textarea')
    expect(widget(item('pregnant'), undefined)).toBe('radio')
  })
})
