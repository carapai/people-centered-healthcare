import type { Bundle, Questionnaire } from '@health/fhir-types'

// What the forms screen reads; drafts are saved through the platform's forms commands.

/** The resources of a search Bundle. */
export function resources<T>(bundle: Bundle): T[] {
  return (bundle.entry ?? []).flatMap((entry) => (entry.resource === undefined ? [] : [entry.resource as T]))
}

/** The canonical a response names: `url|version`. */
export const canonical = (questionnaire: Questionnaire) => `${questionnaire.url ?? ''}|${questionnaire.version ?? ''}`
