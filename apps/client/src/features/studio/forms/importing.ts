import type { Questionnaire, QuestionnaireItem } from '@health/fhir-types'

import type { FormLayout } from '../../forms/layout'

// Import FHIR JSON (studio.md § The form designer; metadata.md § Importing R4 content; #497): a
// pasted Questionnaire replaces the form. R4 content is converted to R5 first, and each import
// answers a conversion report: what was converted, and what could not be, which blocks the
// import, so nothing unconverted reaches the draft. The draft keeps its own id, url, version and
// status, and the pasted Questionnaire's canonical is kept as its source in `derivedFrom`.

/** The FHIR version a pasted Questionnaire is read as. */
export type FhirVersion = 'R5' | 'R4'

/** One line of the conversion report. `at` is `Questionnaire` or the item's linkId. */
export type ReportEntry =
  | { outcome: 'converted'; code: 'choice'; at: string }
  | { outcome: 'converted'; code: 'open-choice'; at: string }
  | { outcome: 'converted'; code: 'source'; at: string; source: string }
  | { outcome: 'unconverted'; code: 'unknown-element'; at: string; element: string }
  | { outcome: 'unconverted'; code: 'contained'; at: string }
  | { outcome: 'unconverted'; code: 'modifier-extension'; at: string }
  | { outcome: 'unconverted'; code: 'unknown-type'; at: string; type: string }

/** Why a paste is not read at all. */
export type ImportRefusal = 'not-json' | 'not-questionnaire' | 'looks-r4'

export type ImportResult = { ok: true; questionnaire: Questionnaire; report: ReportEntry[] } | { ok: false; refusal: ImportRefusal }

// The elements of an R5 Questionnaire and of its items. R4's are a subset of these, so an
// element outside the list is one neither version knows, or one this import cannot carry.
const QUESTIONNAIRE_ELEMENTS = new Set([
  'resourceType', 'id', 'meta', 'implicitRules', 'language', 'text', 'contained', 'extension', 'modifierExtension',
  'url', 'identifier', 'version', 'versionAlgorithmString', 'versionAlgorithmCoding', 'name', 'title', 'derivedFrom', 'status',
  'experimental', 'subjectType', 'date', 'publisher', 'contact', 'description', 'useContext', 'jurisdiction', 'purpose',
  'copyright', 'copyrightLabel', 'approvalDate', 'lastReviewDate', 'effectivePeriod', 'code', 'item',
])
const ITEM_ELEMENTS = new Set([
  'id', 'extension', 'modifierExtension', 'linkId', 'definition', 'code', 'prefix', 'text', 'type', 'enableWhen', 'enableBehavior',
  'disabledDisplay', 'required', 'repeats', 'readOnly', 'maxLength', 'answerConstraint', 'answerValueSet', 'answerOption', 'initial', 'item',
])
const R5_TYPES = new Set(['group', 'display', 'boolean', 'decimal', 'integer', 'date', 'dateTime', 'time', 'string', 'text', 'url', 'coding', 'attachment', 'reference', 'quantity'])
// R4's two choice types became `coding`, with `answerConstraint` saying whether free text is allowed.
const R4_CHOICES: Record<string, 'optionsOnly' | 'optionsOrString'> = { choice: 'optionsOnly', 'open-choice': 'optionsOrString' }

/** An element name with its primitive extension's `_` removed. */
const bare = (key: string) => (key.startsWith('_') ? key.slice(1) : key)

/** The objects in a JSON array, or none. */
const itemsOf = (items: unknown): Record<string, unknown>[] => (Array.isArray(items) ? items.filter((x): x is Record<string, unknown> => typeof x === 'object' && x !== null) : [])

/** Reads `text` as a Questionnaire in `version`, converting R4 to R5, with its report. */
export function readQuestionnaire(text: string, version: FhirVersion): ImportResult {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return { ok: false, refusal: 'not-json' }
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value) || (value as { resourceType?: unknown }).resourceType !== 'Questionnaire') return { ok: false, refusal: 'not-questionnaire' }
  const source = value as Record<string, unknown>
  const report: ReportEntry[] = []
  const r4Types = new Set(Object.keys(R4_CHOICES))

  // An R5 paste holding R4's choice types was meant to be read as R4.
  if (version === 'R5') {
    const walk = (items: unknown): boolean => itemsOf(items).some((item) => r4Types.has(String(item.type)) || walk(item.item))
    if (walk(source.item)) return { ok: false, refusal: 'looks-r4' }
  }

  for (const key of Object.keys(source)) if (!QUESTIONNAIRE_ELEMENTS.has(bare(key))) report.push({ outcome: 'unconverted', code: 'unknown-element', at: 'Questionnaire', element: key })
  if (Array.isArray(source.contained) && source.contained.length > 0) report.push({ outcome: 'unconverted', code: 'contained', at: 'Questionnaire' })
  if (Array.isArray(source.modifierExtension) && source.modifierExtension.length > 0) report.push({ outcome: 'unconverted', code: 'modifier-extension', at: 'Questionnaire' })

  const convert = (items: unknown): QuestionnaireItem[] =>
    itemsOf(items).map((item) => {
      const at = typeof item.linkId === 'string' && item.linkId !== '' ? item.linkId : '?'
      for (const key of Object.keys(item)) if (!ITEM_ELEMENTS.has(bare(key))) report.push({ outcome: 'unconverted', code: 'unknown-element', at, element: key })
      if (Array.isArray(item.modifierExtension) && item.modifierExtension.length > 0) report.push({ outcome: 'unconverted', code: 'modifier-extension', at })
      const out: Record<string, unknown> = { ...item }
      const type = String(item.type)
      const constraint = version === 'R4' ? R4_CHOICES[type] : undefined
      if (constraint !== undefined) {
        out.type = 'coding'
        // An answerConstraint already there (an R5 element in R4 content) is left as it says.
        if (out.answerConstraint === undefined) out.answerConstraint = constraint
        report.push({ outcome: 'converted', code: type === 'choice' ? 'choice' : 'open-choice', at })
      } else if (!R5_TYPES.has(type)) report.push({ outcome: 'unconverted', code: 'unknown-type', at, type })
      if (item.item !== undefined) out.item = convert(item.item)
      return out as unknown as QuestionnaireItem
    })

  const questionnaire = { ...source, item: convert(source.item) } as unknown as Questionnaire
  if (questionnaire.item?.length === 0) delete questionnaire.item
  return { ok: true, questionnaire, report }
}

/** Whether the report blocks the import: something could not be converted. */
export const blocks = (report: readonly ReportEntry[]) => report.some((entry) => entry.outcome === 'unconverted')

/**
 * The draft's Questionnaire replaced by `imported`: the draft keeps its id, url, version and
 * status, and the imported one's canonical, when it is another, is kept in `derivedFrom` as its
 * source. Answers the report with that line added.
 */
export function replaceWith(draft: Questionnaire, imported: Questionnaire, report: readonly ReportEntry[]): { questionnaire: Questionnaire; report: ReportEntry[] } {
  const source = imported.url === undefined ? undefined : imported.version === undefined ? imported.url : `${imported.url}|${imported.version}`
  const fromElsewhere = source !== undefined && imported.url !== draft.url
  const derivedFrom = fromElsewhere ? [...(imported.derivedFrom ?? []).filter((x) => x !== source), source] : imported.derivedFrom
  const { id: _id, url: _url, version: _version, status: _status, derivedFrom: _derived, ...rest } = imported
  const questionnaire: Questionnaire = {
    ...rest,
    ...(draft.id === undefined ? {} : { id: draft.id }),
    ...(draft.url === undefined ? {} : { url: draft.url }),
    ...(draft.version === undefined ? {} : { version: draft.version }),
    status: draft.status,
    ...(derivedFrom === undefined || derivedFrom.length === 0 ? {} : { derivedFrom }),
  }
  return { questionnaire, report: fromElsewhere ? [...report, { outcome: 'converted', code: 'source', at: 'Questionnaire', source }] : [...report] }
}

/** The layout keeping only the top-level questions `questionnaire` still has, and their settings. */
export function pruneLayout(layout: FormLayout | undefined, questionnaire: Questionnaire): FormLayout | undefined {
  if (layout === undefined) return undefined
  const all = new Set<string>()
  const walk = (items: QuestionnaireItem[] | undefined) =>
    (items ?? []).forEach((item) => {
      all.add(item.linkId)
      walk(item.item)
    })
  walk(questionnaire.item)
  const top = new Set((questionnaire.item ?? []).map((item) => item.linkId))
  const { items, ...rest } = layout
  const kept = Object.fromEntries(Object.entries(items ?? {}).filter(([linkId]) => all.has(linkId)))
  return {
    ...rest,
    sections: layout.sections.map((section) => ({ ...section, items: section.items.filter((linkId) => top.has(linkId)) })),
    ...(Object.keys(kept).length === 0 ? {} : { items: kept }),
  }
}
