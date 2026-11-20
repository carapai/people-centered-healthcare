import type { Questionnaire, QuestionnaireItem } from '@health/fhir-types'

import { FORM_LAYOUT_SCHEMA, fitting, type FormLayout, type Widget } from '../../forms/layout'
import { suggestWidget } from '../terminology/model'

// The form designer's model (studio.md § The form designer; #497), as pure functions. It edits
// the draft's own Questionnaire and its Form Layout in place, keeping the two apart: the
// Questionnaire holds what is asked (clinical, approved with the package's kind), the layout
// how it is shown. Whatever the designer does not edit (extensions, expressions, nested groups'
// own settings) is kept as it was read.

/** Where an item is: its index among its siblings, from the top level down. */
export type Path = readonly number[]

/** The answer types the designer offers; `coding` is a choice from the item's options. */
export const ANSWER_TYPES = ['string', 'text', 'integer', 'decimal', 'boolean', 'date', 'dateTime', 'coding'] as const
export type AnswerType = (typeof ANSWER_TYPES)[number]

/** Every widget a Form Layout may name. */
export const WIDGETS: readonly Widget[] = ['input', 'textarea', 'radio', 'select', 'checkbox']

/** One row of the outline: an item, where it is and how deep. */
export interface Row {
  path: Path
  item: QuestionnaireItem
  depth: number
}

/** Every item, depth first, in the order the form shows them. */
export function rows(questionnaire: Questionnaire): Row[] {
  const out: Row[] = []
  const walk = (items: QuestionnaireItem[] | undefined, at: Path) =>
    (items ?? []).forEach((item, index) => {
      const path = [...at, index]
      out.push({ path, item, depth: at.length })
      walk(item.item, path)
    })
  walk(questionnaire.item, [])
  return out
}

/** The item at `path`, if there is one. */
export function itemAt(questionnaire: Questionnaire, path: Path): QuestionnaireItem | undefined {
  let items = questionnaire.item
  let found: QuestionnaireItem | undefined
  for (const index of path) {
    found = items?.[index]
    if (found === undefined) return undefined
    items = found.item
  }
  return found
}

/** `items` with the siblings at `parent` replaced by `change(siblings)`. */
function withSiblings(items: QuestionnaireItem[], parent: Path, change: (siblings: QuestionnaireItem[]) => QuestionnaireItem[]): QuestionnaireItem[] {
  const [head, ...rest] = parent
  if (head === undefined) return change(items)
  return items.map((item, index) => (index === head ? { ...item, item: withSiblings(item.item ?? [], rest, change) } : item))
}

function siblingsChanged(questionnaire: Questionnaire, parent: Path, change: (siblings: QuestionnaireItem[]) => QuestionnaireItem[]): Questionnaire {
  return { ...questionnaire, item: withSiblings(questionnaire.item ?? [], parent, change) }
}

/** The questionnaire with the item at `path` replaced by `change(item)`. */
export function updateItem(questionnaire: Questionnaire, path: Path, change: (item: QuestionnaireItem) => QuestionnaireItem): Questionnaire {
  const index = path[path.length - 1]
  if (index === undefined || itemAt(questionnaire, path) === undefined) return questionnaire
  return siblingsChanged(questionnaire, path.slice(0, -1), (siblings) => siblings.map((item, at) => (at === index ? change(item) : item)))
}

/** Moves the item at `path` by `by` places among its siblings; answers where it went. */
export function moveItem(questionnaire: Questionnaire, path: Path, by: number): { questionnaire: Questionnaire; path: Path } {
  const index = path[path.length - 1]
  const parent = path.slice(0, -1)
  const siblings = parent.length === 0 ? questionnaire.item : itemAt(questionnaire, parent)?.item
  if (index === undefined || siblings === undefined) return { questionnaire, path }
  const to = index + by
  if (to < 0 || to >= siblings.length) return { questionnaire, path }
  const moved = siblingsChanged(questionnaire, parent, (items) => {
    const next = [...items]
    const [item] = next.splice(index, 1)
    if (item !== undefined) next.splice(to, 0, item)
    return next
  })
  return { questionnaire: moved, path: [...parent, to] }
}

/** Every linkId in the form, nested ones too. */
export function linkIds(questionnaire: Questionnaire | undefined): Set<string> {
  return new Set(questionnaire === undefined ? [] : rows(questionnaire).map((row) => row.item.linkId))
}

/** A new top-level question after the others, with an id no item has; answers where it is. */
export function addQuestion(questionnaire: Questionnaire, text: string): { questionnaire: Questionnaire; path: Path } {
  const taken = linkIds(questionnaire)
  let n = (questionnaire.item ?? []).length + 1
  while (taken.has(`question-${n}`)) n += 1
  const item: QuestionnaireItem = { linkId: `question-${n}`, text, type: 'string' }
  const items = [...(questionnaire.item ?? []), item]
  return { questionnaire: { ...questionnaire, item: items }, path: [items.length - 1] }
}

/** The questionnaire without the item at `path`, and without conditions on it. */
export function removeItem(questionnaire: Questionnaire, path: Path): Questionnaire {
  const index = path[path.length - 1]
  const gone = itemAt(questionnaire, path)
  if (index === undefined || gone === undefined) return questionnaire
  const removed = siblingsChanged(questionnaire, path.slice(0, -1), (siblings) => siblings.filter((_, at) => at !== index))
  return mapItems(removed, (item) => {
    if (item.enableWhen === undefined || !item.enableWhen.some((when) => when.question === gone.linkId)) return item
    const { enableWhen: _dropped, ...rest } = item
    const kept = item.enableWhen.filter((when) => when.question !== gone.linkId)
    return kept.length === 0 ? rest : { ...rest, enableWhen: kept }
  })
}

function mapItems(questionnaire: Questionnaire, change: (item: QuestionnaireItem) => QuestionnaireItem): Questionnaire {
  const walk = (items: QuestionnaireItem[]): QuestionnaireItem[] =>
    items.map((item) => {
      const changed = change(item)
      return changed.item === undefined ? changed : { ...changed, item: walk(changed.item) }
    })
  return questionnaire.item === undefined ? questionnaire : { ...questionnaire, item: walk(questionnaire.item) }
}

/**
 * Gives the item at `path` the id `linkId`, and every condition and layout entry that named its
 * old id follows it. A published id is fixed: answers already recorded and mappings refer to
 * it, so the form comes back unchanged.
 */
export function renameItem(form: { questionnaire: Questionnaire; layout: FormLayout | undefined }, path: Path, linkId: string, published: ReadonlySet<string>): { questionnaire: Questionnaire; layout: FormLayout | undefined } {
  const old = itemAt(form.questionnaire, path)?.linkId
  if (old === undefined || old === linkId || published.has(old)) return form
  const renamed = updateItem(form.questionnaire, path, (item) => ({ ...item, linkId }))
  const questionnaire = mapItems(renamed, (item) =>
    item.enableWhen?.some((when) => when.question === old) === true ? { ...item, enableWhen: item.enableWhen.map((when) => (when.question === old ? { ...when, question: linkId } : when)) } : item,
  )
  const layout = form.layout
  if (layout === undefined) return { questionnaire, layout }
  const { [old]: entry, ...others } = layout.items ?? {}
  return {
    questionnaire,
    layout: {
      ...layout,
      sections: layout.sections.map((section) => ({ ...section, items: section.items.map((id) => (id === old ? linkId : id)) })),
      ...(layout.items === undefined ? {} : { items: entry === undefined ? others : { ...others, [linkId]: entry } }),
    },
  }
}

/** The item with answer type `type`: options are kept only for a choice. */
export function withType(item: QuestionnaireItem, type: AnswerType): QuestionnaireItem {
  if (type === 'coding') return { ...item, type }
  const { answerOption: _options, answerValueSet: _valueSet, ...rest } = item
  return { ...rest, type }
}

/** The item with `flag` set, or the property removed when it is off. */
export function withFlag(item: QuestionnaireItem, flag: 'required' | 'repeats', on: boolean): QuestionnaireItem {
  const { [flag]: _old, ...rest } = item
  return on ? { ...rest, [flag]: true } : rest
}

/** An option as the designer edits it. */
export interface Option {
  code: string
  display: string
}

/** The item's coded options. */
export function options(item: QuestionnaireItem): Option[] {
  return (item.answerOption ?? []).flatMap((option) => (option.valueCoding?.code === undefined ? [] : [{ code: option.valueCoding.code, display: option.valueCoding.display ?? '' }]))
}

/** The item with an option added, its code unique among the item's options. */
export function addOption(item: QuestionnaireItem, display: string): QuestionnaireItem {
  const codes = new Set(options(item).map((option) => option.code))
  let n = codes.size + 1
  while (codes.has(`option-${n}`)) n += 1
  // A new option takes the code system its siblings use, if any.
  const system = item.answerOption?.find((option) => option.valueCoding?.system !== undefined)?.valueCoding?.system
  return { ...item, answerOption: [...(item.answerOption ?? []), { valueCoding: { ...(system === undefined ? {} : { system }), code: `option-${n}`, display } }] }
}

/** The item with option `index` changed. */
export function editOption(item: QuestionnaireItem, index: number, change: Partial<Option>): QuestionnaireItem {
  return {
    ...item,
    answerOption: (item.answerOption ?? []).map((option, at) => (at === index && option.valueCoding !== undefined ? { ...option, valueCoding: { ...option.valueCoding, ...change } } : option)),
  }
}

/** The item without option `index`. */
export function removeOption(item: QuestionnaireItem, index: number): QuestionnaireItem {
  const kept = (item.answerOption ?? []).filter((_, at) => at !== index)
  const { answerOption: _old, ...rest } = item
  return kept.length === 0 ? rest : { ...rest, answerOption: kept }
}

/** The item with option `index` moved by `by` places. */
export function moveOption(item: QuestionnaireItem, index: number, by: number): QuestionnaireItem {
  const list = [...(item.answerOption ?? [])]
  const to = index + by
  if (to < 0 || to >= list.length) return item
  const [option] = list.splice(index, 1)
  if (option !== undefined) list.splice(to, 0, option)
  return { ...item, answerOption: list }
}

/**
 * The item bound to the ValueSet `binding` (`url|version`, studio.md § Terminology, Binding from
 * the form designer), or unbound (`null`). A question offers either its own options or a
 * ValueSet's codes, never both, so binding replaces its own options.
 */
export function withValueSet(item: QuestionnaireItem, binding: string | null): QuestionnaireItem {
  const { answerOption: _options, answerValueSet: _valueSet, ...rest } = item
  return binding === null ? rest : { ...rest, answerValueSet: binding }
}

/** A binding's canonical URL and version (none when it names no version). */
export function splitBinding(binding: string): { url: string; version: string | null } {
  const bar = binding.indexOf('|')
  return bar === -1 ? { url: binding, version: null } : { url: binding.slice(0, bar), version: binding.slice(bar + 1) }
}

/** The ValueSets the form binds, each once, in the order they are first asked. */
export function boundValueSets(questionnaire: Questionnaire): string[] {
  return [...new Set(rows(questionnaire).flatMap((row) => (row.item.answerValueSet === undefined ? [] : [row.item.answerValueSet])))]
}

/**
 * The suggested widget for a ValueSet of `codes` codes, as a Form Layout can name it. A Form
 * Layout has no search-as-you-type widget yet, so beyond 30 codes it is a drop-down.
 */
export function layoutWidget(codes: number): Widget {
  const suggested = suggestWidget(codes)
  return suggested === 'search' ? 'select' : suggested
}

/**
 * When the item is shown: always (`null`), only when a yes/no question is answered Yes (its
 * linkId), or under conditions the designer does not edit yet (`'other'`, kept as they are).
 */
export function shownWhen(item: QuestionnaireItem): string | null | 'other' {
  const when = item.enableWhen
  if (when === undefined || when.length === 0) return null
  const [only] = when
  if (when.length === 1 && only !== undefined && only.operator === '=' && only.answerBoolean === true) return only.question
  return 'other'
}

/** The item shown always, or only when yes/no question `question` is answered Yes. */
export function withShownWhen(item: QuestionnaireItem, question: string | null): QuestionnaireItem {
  const { enableWhen: _old, enableBehavior: _behavior, ...rest } = item
  return question === null ? rest : { ...rest, enableWhen: [{ question, operator: '=', answerBoolean: true }] }
}

/** `url|version` of the Questionnaire, as a Form Layout names it. */
export const canonical = (questionnaire: Questionnaire) => `${questionnaire.url ?? ''}|${questionnaire.version ?? ''}`

/** An empty layout for `questionnaire`, before any section, widget or hint is chosen. */
export function blankLayout(questionnaire: Questionnaire): FormLayout {
  const url = questionnaire.url ?? ''
  return {
    $schema: FORM_LAYOUT_SCHEMA,
    url: url.includes('/Questionnaire/') ? url.replace('/Questionnaire/', '/FormLayout/') : `${url}-layout`,
    version: questionnaire.version ?? '',
    questionnaire: canonical(questionnaire),
    sections: [],
  }
}

/** Whether a layout says anything: a section, a widget or a hint. */
export const laysOut = (layout: FormLayout) => layout.sections.length > 0 || Object.keys(layout.items ?? {}).length > 0

/** The layout pointed at the questionnaire's current `url|version`, so the renderer uses it. */
export function aligned(layout: FormLayout, questionnaire: Questionnaire): FormLayout {
  return layout.questionnaire === canonical(questionnaire) ? layout : { ...layout, questionnaire: canonical(questionnaire) }
}

/** The index of the section that places `linkId`, or null. */
export function sectionOf(layout: FormLayout, linkId: string): number | null {
  const at = layout.sections.findIndex((section) => section.items.includes(linkId))
  return at === -1 ? null : at
}

/** The layout with `linkId` placed at the end of section `index`, or in none (`null`). */
export function withSection(layout: FormLayout, linkId: string, index: number | null): FormLayout {
  return {
    ...layout,
    sections: layout.sections.map((section, at) => {
      const items = section.items.filter((id) => id !== linkId)
      return { ...section, items: at === index ? [...items, linkId] : items }
    }),
  }
}

/** The layout with a new, empty section titled `title` at the end. */
export function addSection(layout: FormLayout, title: string): FormLayout {
  return { ...layout, sections: [...layout.sections, { title, items: [] }] }
}

/** The layout with section `index` retitled. */
export function renameSection(layout: FormLayout, index: number, title: string): FormLayout {
  return { ...layout, sections: layout.sections.map((section, at) => (at === index ? { ...section, title } : section)) }
}

/** The layout with section `index` moved by `by` places. */
export function moveSection(layout: FormLayout, index: number, by: number): FormLayout {
  const list = [...layout.sections]
  const to = index + by
  if (to < 0 || to >= list.length) return layout
  const [section] = list.splice(index, 1)
  if (section !== undefined) list.splice(to, 0, section)
  return { ...layout, sections: list }
}

/** The layout without section `index`: its questions are in no section. */
export function removeSection(layout: FormLayout, index: number): FormLayout {
  return { ...layout, sections: layout.sections.filter((_, at) => at !== index) }
}

/** The layout with `linkId`'s widget or hint set, or removed when empty. */
export function withPresentation(layout: FormLayout, linkId: string, change: { widget?: Widget | null; hint?: string }): FormLayout {
  const current = layout.items?.[linkId] ?? {}
  const next: { widget?: Widget; hint?: string } = { ...current }
  if (change.widget !== undefined) {
    if (change.widget === null) delete next.widget
    else next.widget = change.widget
  }
  if (change.hint !== undefined) {
    if (change.hint === '') delete next.hint
    else next.hint = change.hint
  }
  const { [linkId]: _old, ...others } = layout.items ?? {}
  const items = Object.keys(next).length === 0 ? others : { ...others, [linkId]: next }
  const { items: _items, ...rest } = layout
  return Object.keys(items).length === 0 ? rest : { ...rest, items }
}

/** Something to fix, found while editing; *Check now* remains the authority. */
export type Problem =
  | { code: 'no-id'; path: Path }
  | { code: 'duplicate-id'; path: Path; linkId: string }
  | { code: 'no-text'; path: Path; linkId: string }
  | { code: 'no-options'; path: Path; linkId: string }
  | { code: 'condition-not-yes-no'; path: Path; linkId: string; on: string }
  | { code: 'widget-misfit'; path: Path; linkId: string; widget: Widget }
  | { code: 'no-section'; path: Path; linkId: string }
  | { code: 'published-removed'; linkId: string }
  | { code: 'section-no-title'; section: number }

/**
 * The problems in the form as it stands: duplicate or empty ids, a question with no text, a
 * choice with no options, a condition on a question that is not yes/no, a widget that does not
 * fit, a question in no section (once there are sections), a published question removed, and a
 * section with no title.
 */
export function problems(questionnaire: Questionnaire, layout: FormLayout | undefined, published: ReadonlySet<string>): Problem[] {
  const out: Problem[] = []
  const all = rows(questionnaire)
  const byId = new Map<string, QuestionnaireItem>()
  const seen = new Set<string>()
  for (const { path, item } of all) {
    const linkId = item.linkId
    if (linkId.trim() === '') out.push({ code: 'no-id', path })
    else if (seen.has(linkId)) out.push({ code: 'duplicate-id', path, linkId })
    seen.add(linkId)
    if (!byId.has(linkId)) byId.set(linkId, item)
  }
  for (const { path, item } of all) {
    const linkId = item.linkId
    if (item.type !== 'display' && (item.text ?? '').trim() === '') out.push({ code: 'no-text', path, linkId })
    if (item.type === 'coding' && (item.answerOption ?? []).length === 0 && item.answerValueSet === undefined) out.push({ code: 'no-options', path, linkId })
    const on = shownWhen(item)
    if (on !== null && on !== 'other' && byId.get(on)?.type !== 'boolean') out.push({ code: 'condition-not-yes-no', path, linkId, on })
    const chosen = layout?.items?.[linkId]?.widget
    if (chosen !== undefined && !fitting(item).includes(chosen)) out.push({ code: 'widget-misfit', path, linkId, widget: chosen })
    if (layout !== undefined && layout.sections.length > 0 && path.length === 1 && sectionOf(layout, linkId) === null) out.push({ code: 'no-section', path, linkId })
  }
  for (const linkId of published) if (!byId.has(linkId)) out.push({ code: 'published-removed', linkId })
  layout?.sections.forEach((section, n) => {
    if (section.title.trim() === '') out.push({ code: 'section-no-title', section: n })
  })
  return out
}

/**
 * Moves the item at `path` by `by` places as the outline shows it: a nested question among its
 * siblings; a top-level one placed in a section within that section (the layout's order, which
 * is the order it is shown in); and one in no section among the others in no section.
 */
export function moveShown(form: { questionnaire: Questionnaire; layout: FormLayout | undefined }, path: Path, by: number): { questionnaire: Questionnaire; layout: FormLayout | undefined; path: Path } {
  const [index] = path
  const item = itemAt(form.questionnaire, path)
  if (index === undefined || item === undefined) return { ...form, path }
  if (path.length > 1) {
    const moved = moveItem(form.questionnaire, path, by)
    return { questionnaire: moved.questionnaire, layout: form.layout, path: moved.path }
  }
  const layout = form.layout
  const section = layout === undefined ? null : sectionOf(layout, item.linkId)
  if (layout !== undefined && section !== null) {
    const ids = layout.sections[section]?.items ?? []
    const at = ids.indexOf(item.linkId)
    const to = at + by
    if (to < 0 || to >= ids.length) return { ...form, path }
    const next = [...ids]
    next.splice(at, 1)
    next.splice(to, 0, item.linkId)
    return { questionnaire: form.questionnaire, layout: { ...layout, sections: layout.sections.map((s, n) => (n === section ? { ...s, items: next } : s)) }, path }
  }
  // Among the top-level questions in no section, in the Questionnaire's order.
  const loose = (form.questionnaire.item ?? []).flatMap((x, n) => (layout !== undefined && sectionOf(layout, x.linkId) !== null ? [] : [n]))
  const target = loose[loose.indexOf(index) + by]
  if (target === undefined) return { ...form, path }
  const moved = moveItem(form.questionnaire, path, target - index)
  return { questionnaire: moved.questionnaire, layout, path: moved.path }
}

/** A group of the outline: a section's top-level questions in its order, or those in none. */
export interface OutlineGroup {
  /** The section's index in the layout, or null for questions in no section. */
  section: number | null
  title: string | null
  rows: Row[]
}

/** The outline: each section's questions, then those in no section, each followed by its nested ones. */
export function outline(questionnaire: Questionnaire, layout: FormLayout | undefined): OutlineGroup[] {
  const all = rows(questionnaire)
  const under = (top: number) => all.filter((row) => row.path[0] === top)
  const top = questionnaire.item ?? []
  const indexOf = new Map(top.map((item, n) => [item.linkId, n]))
  const placed = new Set<number>()
  const groups: OutlineGroup[] = (layout?.sections ?? []).map((section, n) => ({
    section: n,
    title: section.title,
    rows: section.items.flatMap((linkId) => {
      const at = indexOf.get(linkId)
      if (at === undefined || placed.has(at)) return []
      placed.add(at)
      return under(at)
    }),
  }))
  groups.push({ section: null, title: null, rows: top.flatMap((_, n) => (placed.has(n) ? [] : under(n))) })
  return groups
}
