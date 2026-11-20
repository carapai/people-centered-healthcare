import type { Questionnaire } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import { sections } from '../src/features/forms/layout'
import {
  addOption,
  addQuestion,
  addSection,
  aligned,
  blankLayout,
  editOption,
  itemAt,
  linkIds,
  moveItem,
  moveOption,
  moveShown,
  outline,
  options,
  problems,
  removeItem,
  removeOption,
  renameSection,
  renameItem,
  rows,
  sectionOf,
  shownWhen,
  updateItem,
  withFlag,
  withPresentation,
  withSection,
  withShownWhen,
  withType,
} from '../src/features/studio/forms/model'

// The form designer's model (#497): it edits the draft's Questionnaire and its Form Layout in
// place, keeping what is asked apart from how it is shown.

const form: Questionnaire = {
  resourceType: 'Questionnaire',
  url: 'https://health.go.ug/fhir/Questionnaire/anc-visit',
  version: '1.1.0',
  status: 'draft',
  extension: [{ url: 'https://example.org/kept', valueString: 'kept as read' }],
  item: [
    { linkId: 'pregnant', text: 'Pregnant', type: 'boolean' },
    { linkId: 'lmp', text: 'Last menstrual period', type: 'date', enableWhen: [{ question: 'pregnant', operator: '=', answerBoolean: true }] },
    {
      linkId: 'vitals',
      text: 'Vital signs',
      type: 'group',
      item: [
        { linkId: 'pulse', text: 'Pulse', type: 'integer' },
        { linkId: 'temperature', text: 'Temperature', type: 'decimal' },
      ],
    },
    {
      linkId: 'category',
      text: 'Category',
      type: 'coding',
      answerOption: [
        { valueCoding: { system: 'https://health.go.ug/fhir/CodeSystem/category', code: 'a', display: 'A' } },
        { valueCoding: { system: 'https://health.go.ug/fhir/CodeSystem/category', code: 'b', display: 'B' } },
      ],
    },
  ],
}

const none = new Set<string>()

describe('the form designer', () => {
  test('editing a question’s text changes the Questionnaire the preview renders, and nothing else', () => {
    const edited = updateItem(form, [2, 0], (item) => ({ ...item, text: 'Pulse (beats/min)' }))
    expect(itemAt(edited, [2, 0])?.text).toBe('Pulse (beats/min)')
    // The preview is keyed on the Questionnaire, so a changed one re-renders at once.
    expect(JSON.stringify(edited)).not.toBe(JSON.stringify(form))
    expect(edited.extension).toEqual(form.extension)
    expect(itemAt(form, [2, 0])?.text).toBe('Pulse')
  })

  test('a choice question with no options is listed as a problem', () => {
    const empty = updateItem(form, [3], (item) => removeOption(removeOption(item, 0), 0))
    expect(problems(empty, undefined, none)).toEqual([{ code: 'no-options', path: [3], linkId: 'category' }])
    const added = updateItem(empty, [3], (item) => addOption(item, 'C'))
    expect(problems(added, undefined, none)).toEqual([])
    // A new question is plain text; made a choice, it has no options yet.
    const { questionnaire, path } = addQuestion(form, 'New question')
    const choice = updateItem(questionnaire, path, (item) => withType(item, 'coding'))
    expect(problems(choice, undefined, none)).toContainEqual({ code: 'no-options', path: [4], linkId: 'question-5' })
  })

  test('a published item’s id cannot be changed, and a new one’s can, with what refers to it', () => {
    const published = linkIds(form)
    const kept = renameItem({ questionnaire: form, layout: undefined }, [0], 'is-pregnant', published)
    expect(kept.questionnaire).toBe(form)

    const layout = withPresentation(withSection(addSection(blankLayout(form), 'Pregnancy'), 'pregnant', 0), 'pregnant', { hint: 'Ask first' })
    const renamed = renameItem({ questionnaire: form, layout }, [0], 'is-pregnant', none)
    expect(itemAt(renamed.questionnaire, [0])?.linkId).toBe('is-pregnant')
    expect(itemAt(renamed.questionnaire, [1])?.enableWhen?.[0]?.question).toBe('is-pregnant')
    expect(renamed.layout?.sections[0]?.items).toEqual(['is-pregnant'])
    expect(renamed.layout?.items).toEqual({ 'is-pregnant': { hint: 'Ask first' } })
  })

  test('a published question removed is listed, and conditions on it go with it', () => {
    const removed = removeItem(form, [0])
    expect(itemAt(removed, [0])?.linkId).toBe('lmp')
    expect(itemAt(removed, [0])?.enableWhen).toBeUndefined()
    expect(problems(removed, undefined, linkIds(form))).toEqual([{ code: 'published-removed', linkId: 'pregnant' }])
  })

  test('questions and options reorder among their siblings', () => {
    const down = moveItem(form, [0], 1)
    expect(down.path).toEqual([1])
    expect(rows(down.questionnaire).map((row) => row.item.linkId)).toEqual(['lmp', 'pregnant', 'vitals', 'pulse', 'temperature', 'category'])
    const nested = moveItem(form, [2, 1], -1)
    expect(nested.path).toEqual([2, 0])
    expect(itemAt(nested.questionnaire, [2, 0])?.linkId).toBe('temperature')
    // The ends stay put.
    expect(moveItem(form, [0], -1).questionnaire).toBe(form)
    const swapped = updateItem(form, [3], (item) => moveOption(editOption(item, 0, { display: 'Alpha' }), 0, 1))
    expect(options(itemAt(swapped, [3]) ?? { linkId: '', type: '' })).toEqual([
      { code: 'b', display: 'B' },
      { code: 'a', display: 'Alpha' },
    ])
    // A new option takes its siblings' code system and a code of its own.
    expect(itemAt(updateItem(form, [3], (item) => addOption(item, 'C')), [3])?.answerOption?.[2]).toEqual({ valueCoding: { system: 'https://health.go.ug/fhir/CodeSystem/category', code: 'option-3', display: 'C' } })
  })

  test('the Question group: type, required, more than one answer and shown only when', () => {
    const item = itemAt(form, [3])
    if (item === undefined) throw new Error('no item')
    expect(withType(item, 'string').answerOption).toBeUndefined()
    expect(withFlag(withFlag(item, 'required', true), 'repeats', true)).toMatchObject({ required: true, repeats: true })
    expect('required' in withFlag({ ...item, required: true }, 'required', false)).toBe(false)
    expect(shownWhen(itemAt(form, [1]) ?? item)).toBe('pregnant')
    expect(shownWhen(item)).toBeNull()
    expect(shownWhen({ ...item, enableWhen: [{ question: 'pulse', operator: 'exists', answerBoolean: true }] })).toBe('other')
    expect(withShownWhen(item, 'pregnant').enableWhen).toEqual([{ question: 'pregnant', operator: '=', answerBoolean: true }])
    // A condition on a question that is not yes/no is a problem.
    const wrong = updateItem(form, [3], (x) => withShownWhen(x, 'pulse'))
    expect(problems(wrong, undefined, none)).toEqual([{ code: 'condition-not-yes-no', path: [3], linkId: 'category', on: 'pulse' }])
  })

  test('the Layout group: sections, widgets and hints, and what does not fit', () => {
    let layout = addSection(addSection(blankLayout(form), 'Pregnancy'), 'Findings')
    expect(layout).toMatchObject({ url: 'https://health.go.ug/fhir/FormLayout/anc-visit', questionnaire: 'https://health.go.ug/fhir/Questionnaire/anc-visit|1.1.0' })
    layout = withSection(withSection(layout, 'pregnant', 0), 'lmp', 0)
    layout = withSection(layout, 'lmp', 1)
    expect(sectionOf(layout, 'lmp')).toBe(1)
    expect(layout.sections.map((s) => s.items)).toEqual([['pregnant'], ['lmp']])
    // Once there are sections, a top-level question in none is listed; it shows at the end.
    expect(problems(form, layout, none).filter((p) => p.code === 'no-section').map((p) => ('linkId' in p ? p.linkId : ''))).toEqual(['vitals', 'category'])
    // The renderer places them as the layout says.
    expect(sections(form, layout).map((s) => s.title)).toEqual(['Pregnancy', 'Findings', undefined])

    layout = withPresentation(layout, 'pulse', { widget: 'checkbox' })
    expect(problems(form, layout, none)).toContainEqual({ code: 'widget-misfit', path: [2, 0], linkId: 'pulse', widget: 'checkbox' })
    layout = withPresentation(withPresentation(layout, 'pulse', { widget: null }), 'pulse', { hint: 'At rest' })
    expect(layout.items).toEqual({ pulse: { hint: 'At rest' } })
    expect(withPresentation(layout, 'pulse', { hint: '' }).items).toBeUndefined()
    // A section needs a title (the layout's schema).
    expect(problems(form, renameSection(layout, 1, ' '), none)).toContainEqual({ code: 'section-no-title', section: 1 })

    // The layout follows the Questionnaire's version, so the renderer still applies it.
    const next = { ...form, version: '1.2.0' }
    expect(aligned(layout, next).questionnaire).toBe('https://health.go.ug/fhir/Questionnaire/anc-visit|1.2.0')
  })

  test('duplicate and empty ids are problems', () => {
    const twice = updateItem(form, [1], (item) => ({ ...item, linkId: 'pregnant' }))
    expect(problems(twice, undefined, none)).toContainEqual({ code: 'duplicate-id', path: [1], linkId: 'pregnant' })
    const blank = updateItem(form, [1], (item) => ({ ...item, linkId: ' ' }))
    expect(problems(blank, undefined, none)).toContainEqual({ code: 'no-id', path: [1] })
  })
})

describe('the form designer’s outline', () => {
  test('questions are grouped by section, those in none last, and move as they are shown', () => {
    const layout = withSection(withSection(addSection(blankLayout(form), 'Findings'), 'category', 0), 'vitals', 0)
    const groups = outline(form, layout)
    expect(groups.map((g) => [g.title, g.rows.map((r) => r.item.linkId)])).toEqual([
      ['Findings', ['category', 'vitals', 'pulse', 'temperature']],
      [null, ['pregnant', 'lmp']],
    ])
    // Within a section, the layout's order changes.
    const inSection = moveShown({ questionnaire: form, layout }, [2], -1)
    expect(inSection.layout?.sections[0]?.items).toEqual(['vitals', 'category'])
    expect(inSection.questionnaire).toBe(form)
    // In no section, it passes the next question in no section.
    const loose = moveShown({ questionnaire: form, layout: withSection(layout, 'lmp', 0) }, [0], 1)
    expect(outline(loose.questionnaire, loose.layout).at(-1)?.rows.map((r) => r.item.linkId)).toEqual(['pregnant'])
    const twoLoose = moveShown({ questionnaire: form, layout }, [0], 1)
    expect(outline(twoLoose.questionnaire, twoLoose.layout).at(-1)?.rows.map((r) => r.item.linkId)).toEqual(['lmp', 'pregnant'])
    expect(twoLoose.path).toEqual([1])
    // Nested questions move among their siblings.
    expect(moveShown({ questionnaire: form, layout }, [2, 0], 1).path).toEqual([2, 1])
  })
})
