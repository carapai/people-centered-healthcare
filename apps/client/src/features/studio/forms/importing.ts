import type { StudioConversionFinding, StudioFhirVersion, StudioImportRefusal, StudioQuestionnaireImport } from '@health/api-client'
import type { Questionnaire, QuestionnaireItem } from '@health/fhir-types'

import type { FormLayout } from '../../forms/layout'

// Import FHIR JSON (studio.md § The form designer; metadata.md § Importing R4 content; #497,
// #853): a pasted Questionnaire replaces the form. Central reads it, converting R4 to R5 with
// `health-fhir-mapping` (ADRs 0015, 0050): no R4 conversion happens here. Each import answers the
// server's conversion report: what was converted, and what could not be, which blocks the import,
// so nothing unconverted reaches the draft. The draft keeps its own id, url, version and status,
// and the pasted Questionnaire's canonical is kept as its source in `derivedFrom`.

/** The FHIR version a pasted Questionnaire is read as. */
export type FhirVersion = StudioFhirVersion

/** Why a paste is not read at all. */
export type ImportRefusal = StudioImportRefusal

/** One line of the report: the server's conversion findings, then the source this import keeps. */
export type ReportEntry = StudioConversionFinding | { outcome: 'converted'; code: 'source'; at: string; source: string }

/** Central's reading of a paste: the R5 Questionnaire and its report, or why it was not read. */
/** `blocked` is the server's: something could not be converted. */
export type ImportResult = { ok: true; questionnaire: Questionnaire; report: StudioConversionFinding[]; blocked: boolean } | { ok: false; refusal: ImportRefusal }

/** The server's answer as an import result. */
export function importResult(answer: StudioQuestionnaireImport): ImportResult {
  if (answer.refusal !== null && answer.refusal !== undefined) return { ok: false, refusal: answer.refusal }
  if (answer.questionnaire === null || answer.questionnaire === undefined) return { ok: false, refusal: 'not-questionnaire' }
  return { ok: true, questionnaire: answer.questionnaire as unknown as Questionnaire, report: answer.report, blocked: answer.blocked }
}

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
