import type { Questionnaire, QuestionnaireItem } from '@health/fhir-types'

// Form Layouts (configuration.md: presentation is kept apart, §42): a configuration artefact
// keyed to a Questionnaire `url|version`. It arranges the Questionnaire's top-level items into
// titled sections and picks widgets and hints; it never changes what is asked. A top-level item
// the layout does not place is shown in a trailing section, so a layout cannot hide a question.

/** The JSON Schema URI of a Form Layout (`$schema`). */
export const FORM_LAYOUT_SCHEMA = 'https://carapai.github.io/people-centered-healthcare/fhir/schema/form-layout/1'

/** How a question is shown. */
export type Widget = 'input' | 'textarea' | 'radio' | 'select' | 'checkbox'

/** A Form Layout. */
export interface FormLayout {
  $schema: typeof FORM_LAYOUT_SCHEMA
  url: string
  version: string
  /** The Questionnaire it lays out, `url|version`. */
  questionnaire: string
  /** Titled sections of top-level linkIds, in order; an `id` lets a Language Pack translate the title. */
  sections: { id?: string; title: string; items: string[] }[]
  /** Per question, by linkId. */
  items?: Record<string, { widget?: Widget; hint?: string }>
}

/** A section as rendered: its title (none for the trailing one) and its items. */
export interface Section {
  title: string | undefined
  items: QuestionnaireItem[]
}

/** The questionnaire's top-level items, sectioned by `layout` (if it is for this Questionnaire). */
export function sections(questionnaire: Questionnaire, layout: FormLayout | undefined): Section[] {
  const top = questionnaire.item ?? []
  const canonical = `${questionnaire.url ?? ''}|${questionnaire.version ?? ''}`
  if (layout === undefined || layout.questionnaire !== canonical) return [{ title: undefined, items: top }]
  const byId = new Map(top.map((item) => [item.linkId, item]))
  const placed = new Set<string>()
  const result: Section[] = layout.sections.map((section) => ({
    title: section.title,
    items: section.items.flatMap((linkId) => {
      const item = byId.get(linkId)
      if (item === undefined || placed.has(linkId)) return []
      placed.add(linkId)
      return [item]
    }),
  }))
  const rest = top.filter((item) => !placed.has(item.linkId))
  if (rest.length > 0) result.push({ title: undefined, items: rest })
  return result.filter((section) => section.items.length > 0)
}

/** The widgets that fit `item`'s type, the default first. */
export function fitting(item: QuestionnaireItem): Widget[] {
  const fits: Record<string, Widget[]> = {
    boolean: ['radio', 'checkbox'],
    coding: ['radio', 'select'],
    string: ['input', 'textarea'],
    text: ['textarea', 'input'],
  }
  return fits[item.type] ?? ['input']
}

/** The most choices shown as radio buttons when the layout names no widget (studio.md § Terminology). */
export const RADIO_MOST = 7

/**
 * The widget for `item`: the layout's choice where it fits the item's type, else the default. A
 * choice question with more than `RADIO_MOST` of its `choices` is a drop-down, as a large bound
 * ValueSet is until a Form Layout has a search widget (#843).
 */
export function widget(item: QuestionnaireItem, layout: FormLayout | undefined, choices = 0): Widget {
  const chosen = layout?.items?.[item.linkId]?.widget
  const allowed = fitting(item)
  if (chosen !== undefined && allowed.includes(chosen)) return chosen
  if (item.type === 'coding' && choices > RADIO_MOST) return 'select'
  return allowed[0] ?? 'input'
}
