// PROTOTYPE (#479), throwaway: one editable form model behind the three form designer variants.
// The Questionnaire (clinical structure), its Form Layout (presentation, form-layout/1) and a
// simplified per-question extraction target are kept apart, as configuration.md and ADR 0022 do.
import type { Questionnaire } from '@health/fhir-types'

import { FORM_LAYOUT_SCHEMA, type FormLayout, type Widget } from '../../forms/layout'

export type ItemType = 'string' | 'text' | 'integer' | 'decimal' | 'boolean' | 'date' | 'choice'
export const ITEM_TYPES: ItemType[] = ['string', 'text', 'integer', 'decimal', 'boolean', 'date', 'choice']
export const WIDGETS: Widget[] = ['input', 'textarea', 'radio', 'select', 'checkbox']

export interface Item {
  linkId: string
  text: string
  type: ItemType
  required: boolean
  repeats: boolean
  options: { code: string; display: string }[]
  /** Shown only when this boolean question is answered "Yes". */
  shownWhen: string | null
  /** The section it is placed in (Form Layout), or null: the trailing section. */
  section: string | null
  widget: Widget | null
  hint: string
  /** Extraction (simplified): the resource and code this answer becomes, or nothing. */
  extract: { resource: 'Observation' | 'Condition' | 'none'; code: string; display: string }
}

export interface Form {
  title: string
  url: string
  version: string
  sections: string[]
  items: Item[]
}

const item = (i: Partial<Item> & Pick<Item, 'linkId' | 'text' | 'type'>): Item => ({
  required: false,
  repeats: false,
  options: [],
  shownWhen: null,
  section: null,
  widget: null,
  hint: '',
  extract: { resource: 'none', code: '', display: '' },
  ...i,
})

export const SAMPLE: Form = {
  title: 'ANC visit',
  url: 'https://health.go.ug/fhir/Questionnaire/anc-visit',
  version: '2.1.0',
  sections: ['Pregnancy', 'Danger signs', 'Measurements'],
  items: [
    item({ linkId: 'lmp', text: 'Last menstrual period', type: 'date', required: true, section: 'Pregnancy', extract: { resource: 'Observation', code: '8665-2', display: 'Last menstrual period start date' } }),
    item({ linkId: 'gestational-age', text: 'Gestational age (weeks)', type: 'integer', required: true, section: 'Pregnancy', hint: 'From the LMP or the ultrasound', extract: { resource: 'Observation', code: '11884-4', display: 'Gestational age Estimated' } }),
    item({ linkId: 'danger-sign-present', text: 'Any danger sign?', type: 'boolean', required: true, section: 'Danger signs', widget: 'radio' }),
    item({
      linkId: 'danger-signs',
      text: 'Danger signs',
      type: 'choice',
      repeats: true,
      shownWhen: 'danger-sign-present',
      section: 'Danger signs',
      widget: 'checkbox',
      options: [
        { code: 'bleeding', display: 'Vaginal bleeding' },
        { code: 'headache', display: 'Severe headache' },
        { code: 'vision', display: 'Blurred vision' },
        { code: 'convulsions', display: 'Convulsions' },
        { code: 'fever', display: 'Fever' },
      ],
      extract: { resource: 'Condition', code: 'anc-danger-sign', display: 'Danger sign in pregnancy' },
    }),
    item({ linkId: 'referred', text: 'Referred today', type: 'boolean', shownWhen: 'danger-sign-present', section: 'Danger signs' }),
    item({ linkId: 'systolic', text: 'Systolic blood pressure (mmHg)', type: 'integer', section: 'Measurements', extract: { resource: 'Observation', code: '8480-6', display: 'Systolic blood pressure' } }),
    item({ linkId: 'weight', text: 'Weight (kg)', type: 'decimal', section: 'Measurements', extract: { resource: 'Observation', code: '29463-7', display: 'Body weight' } }),
    item({ linkId: 'notes', text: 'Notes', type: 'text' }),
  ],
}

/** The FHIR Questionnaire the form model stands for: clinical structure only. */
export function toQuestionnaire(form: Form): Questionnaire {
  return {
    resourceType: 'Questionnaire',
    id: 'prototype-anc-visit',
    url: form.url,
    version: form.version,
    title: form.title,
    status: 'draft',
    item: form.items.map((i) => ({
      linkId: i.linkId,
      text: i.text,
      type: i.type === 'choice' ? 'coding' : i.type,
      ...(i.required ? { required: true } : {}),
      ...(i.repeats ? { repeats: true } : {}),
      ...(i.options.length > 0 ? { answerOption: i.options.map((o) => ({ valueCoding: { system: `${form.url}/codes`, code: o.code, display: o.display } })) } : {}),
      ...(i.shownWhen === null ? {} : { enableWhen: [{ question: i.shownWhen, operator: '=', answerBoolean: true }] }),
    })),
  } as Questionnaire
}

/** Its Form Layout: sections, widgets and hints. A question it does not place still shows. */
export function toLayout(form: Form): FormLayout {
  const items: FormLayout['items'] = {}
  for (const i of form.items) {
    if (i.widget !== null || i.hint !== '') items[i.linkId] = { ...(i.widget === null ? {} : { widget: i.widget }), ...(i.hint === '' ? {} : { hint: i.hint }) }
  }
  return {
    $schema: FORM_LAYOUT_SCHEMA,
    url: `${form.url.replace('/Questionnaire/', '/FormLayout/')}`,
    version: form.version,
    questionnaire: `${form.url}|${form.version}`,
    sections: form.sections.map((title) => ({ title, items: form.items.filter((i) => i.section === title).map((i) => i.linkId) })),
    items,
  }
}

/** The extraction targets (a simplified stand-in for the Extraction Mapping Library). */
export function toMapping(form: Form) {
  return form.items.filter((i) => i.extract.resource !== 'none').map((i) => ({ question: i.linkId, becomes: i.extract.resource, code: `http://loinc.org|${i.extract.code}`, display: i.extract.display }))
}

/** Reads a pasted FHIR Questionnaire into the model (top-level items only, in the prototype). */
export function fromQuestionnaire(json: string, current: Form): Form {
  const q = JSON.parse(json) as Questionnaire
  return {
    title: q.title ?? q.name ?? 'Imported form',
    url: q.url ?? current.url,
    version: q.version ?? current.version,
    sections: [],
    items: (q.item ?? []).map((i) =>
      item({
        linkId: i.linkId,
        text: i.text ?? i.linkId,
        type: (i.type === 'coding' ? 'choice' : (ITEM_TYPES as string[]).includes(i.type) ? i.type : 'string') as ItemType,
        required: i.required === true,
        repeats: i.repeats === true,
        options: (i.answerOption ?? []).flatMap((o) => (o.valueCoding?.code === undefined ? [] : [{ code: o.valueCoding.code, display: o.valueCoding.display ?? o.valueCoding.code }])),
        shownWhen: i.enableWhen?.[0]?.question ?? null,
      }),
    ),
  }
}

/** Problems an author should see while editing (the real ones come from Check now). */
export function problems(form: Form): string[] {
  const out: string[] = []
  const ids = new Set<string>()
  for (const i of form.items) {
    if (ids.has(i.linkId)) out.push(`Two questions share the id "${i.linkId}"`)
    ids.add(i.linkId)
    if (i.type === 'choice' && i.options.length === 0) out.push(`"${i.text}" is a choice with no options`)
    if (i.shownWhen !== null && !form.items.some((x) => x.linkId === i.shownWhen && x.type === 'boolean')) out.push(`"${i.text}" depends on a question that is not a yes/no question`)
    if (i.widget === 'checkbox' && !(i.type === 'boolean' || (i.type === 'choice' && i.repeats))) out.push(`"${i.text}": checkboxes need a yes/no or a multi-answer choice`)
    if (i.section === null) out.push(`"${i.text}" is in no section: it shows at the end of the form`)
  }
  return out
}
