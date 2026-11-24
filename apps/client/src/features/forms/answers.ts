import type { Coding, Questionnaire, ValueSet, ValueSetExpansionContains } from '@health/fhir-types'
import { queryOptions, useQueries } from '@tanstack/react-query'

import { fhirKeys, usePlatform, type FhirClient } from '../../platform'
import { boundValueSets, splitBinding } from './model'

// The codes of a form's bound questions, for forms in use (#843; terminology.md § Runtime
// service): each ValueSet a question binds is expanded by the node the client talks to, central
// or the facility's edge, from its active packages' terminology (the pinned expansion when the
// package ships one), so a bound question offers its codes offline on the edge too.

/** The most codes a bound question offers: a drop-down's worth until there is a search widget. */
export const EXPANDED = 1000

/** Every code of an expansion, nested ones too, leaving out abstract groupers. */
export function codesOf(valueSet: ValueSet): Coding[] {
  const found: Coding[] = []
  const walk = (contains: ValueSetExpansionContains[] | undefined) => {
    for (const entry of contains ?? []) {
      if (entry.code !== undefined && entry.abstract !== true) {
        found.push({
          ...(entry.system === undefined ? {} : { system: entry.system }),
          ...(entry.version === undefined ? {} : { version: entry.version }),
          code: entry.code,
          ...(entry.display === undefined ? {} : { display: entry.display }),
        })
      }
      walk(entry.contains)
    }
  }
  walk(valueSet.expansion?.contains)
  return found
}

/** The codes the ValueSet `binding` (`url|version`) offers on a form: its active codes. */
export function boundCodesQuery(fhir: FhirClient, binding: string) {
  const { url, version } = splitBinding(binding)
  return queryOptions({
    queryKey: [...fhirKeys.type('ValueSet'), '$expand', binding] as const,
    queryFn: async ({ signal }): Promise<Coding[]> =>
      codesOf(await fhir.expand(url, { ...(version === null ? {} : { valueSetVersion: version }), activeOnly: true, count: EXPANDED, signal })),
    // A binding names a version, whose codes do not change while the form is filled.
    staleTime: 5 * 60_000,
    retry: 1,
  })
}

/** What a form's bound questions offer: the codes by binding, and whether any could not be read. */
export interface BoundAnswers {
  answers: Record<string, Coding[]>
  failed: boolean
}

/** The codes of every ValueSet `questionnaire` binds, by binding, for the renderer's `answers`. */
export function useBoundAnswers(questionnaire: Questionnaire | undefined): BoundAnswers {
  const platform = usePlatform()
  const bindings = questionnaire === undefined ? [] : boundValueSets(questionnaire)
  const expansions = useQueries({ queries: bindings.map((binding) => boundCodesQuery(platform.fhir, binding)) })
  return {
    answers: Object.fromEntries(bindings.flatMap((binding, n) => {
      const codes = expansions[n]?.data
      return codes === undefined ? [] : [[binding, codes]]
    })),
    failed: expansions.some((expansion) => expansion.isError),
  }
}
