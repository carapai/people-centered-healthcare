import type { StudioCodeSystem, StudioConcept, StudioConceptPage, StudioExpandInput, StudioExpansion, StudioProblem, StudioValueSet } from '@health/api-client'
import type { ValueSet } from '@health/fhir-types'

import type { Transport } from '../../../platform/types'
import { StudioError, studioKey } from '../api'

// The terminology a package version is authored against (studio.md § Terminology; #523), read
// from `/api/studio/packages/{id}/terminology/` at central. Search and the hierarchy run on the
// server a page at a time, so a large code system such as ICD-11 never loads in the browser.
// The form designer's ValueSet picker (#497) uses `valueSetsQuery` and `expandValueSet` too.

export type { StudioCode, StudioCodeSystem, StudioConcept, StudioConceptPage, StudioConceptRow, StudioExpansion, StudioValueSet } from '@health/api-client'

type Operation = 'listStudioCodeSystems' | 'listStudioConcepts' | 'getStudioConcept' | 'listStudioValueSets' | 'expandStudioValueSet'

async function parsed<T>(operation: Operation, status: number, body: unknown): Promise<T> {
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse(operation, status, body)
  if (status === 200) return value as T
  const problem = value as StudioProblem | null
  throw new StudioError(status, problem?.detail ?? '', problem?.refusal ?? null)
}

const base = (id: string) => `/studio/packages/${encodeURIComponent(id)}/terminology`

const query = (params: Record<string, string | number | undefined>) => {
  const search = new URLSearchParams()
  for (const [name, value] of Object.entries(params)) if (value !== undefined && value !== '') search.set(name, String(value))
  const text = search.toString()
  return text === '' ? '' : `?${text}`
}

/** The query key every terminology read of package version `id` sits under. */
export const terminologyKey = (id: string) => [...studioKey, id, 'terminology'] as const

type Signal = { signal?: AbortSignal }
const withSignal = (signal: AbortSignal | undefined) => (signal === undefined ? {} : { signal })

/** The code systems: this package's own first, then the published ones, local or external. */
export function codeSystemsQuery(transport: Transport, id: string) {
  return {
    queryKey: [...terminologyKey(id), 'code-systems'] as const,
    queryFn: async ({ signal }: Signal = {}): Promise<StudioCodeSystem[]> => {
      const response = await transport.request({ method: 'GET', path: `${base(id)}/code-systems`, ...withSignal(signal) })
      return (await parsed<{ codeSystems: StudioCodeSystem[] }>('listStudioCodeSystems', response.status, response.body)).codeSystems
    },
  }
}

/** Which concepts: those under `parent` (or the top level), or those matching `text`. */
export type ConceptsAsk = { system: string; version?: string | undefined; parent?: string | undefined; text?: string | undefined; offset?: number; count?: number }

/** A page of a code system's concepts, found on the server. */
export function conceptsQuery(transport: Transport, id: string, ask: ConceptsAsk) {
  return {
    queryKey: [...terminologyKey(id), 'concepts', ask] as const,
    queryFn: async ({ signal }: Signal = {}): Promise<StudioConceptPage> => {
      const response = await transport.request({ method: 'GET', path: `${base(id)}/concepts${query(ask)}`, ...withSignal(signal) })
      return parsed<StudioConceptPage>('listStudioConcepts', response.status, response.body)
    },
  }
}

/** One concept: display, definition, parents and the ValueSets that contain it. */
export function conceptQuery(transport: Transport, id: string, system: string, version: string | undefined, code: string) {
  return {
    queryKey: [...terminologyKey(id), 'concept', system, version ?? '', code] as const,
    queryFn: async ({ signal }: Signal = {}): Promise<StudioConcept> => {
      const response = await transport.request({ method: 'GET', path: `${base(id)}/concept${query({ system, version, code })}`, ...withSignal(signal) })
      return parsed<StudioConcept>('getStudioConcept', response.status, response.body)
    },
  }
}

/** The ValueSets a form of package version `id` may bind: its own and the published ones,
 * matching `text`. */
export function valueSetsQuery(transport: Transport, id: string, text = '') {
  return {
    queryKey: [...terminologyKey(id), 'value-sets', text] as const,
    queryFn: async ({ signal }: Signal = {}): Promise<StudioValueSet[]> => {
      const response = await transport.request({ method: 'GET', path: `${base(id)}/value-sets${query({ text })}`, ...withSignal(signal) })
      return (await parsed<{ valueSets: StudioValueSet[] }>('listStudioValueSets', response.status, response.body)).valueSets
    },
  }
}

/** Which ValueSet to expand: one held, by `url|version`, or an unsaved one of this package. */
export type ExpandAsk = { url: string; version?: string | null | undefined } | { valueSet: ValueSet }

/** A ValueSet's expansion, with its change against published (for this package's own) and the
 * form questions that bind it. */
export async function expandValueSet(transport: Transport, id: string, ask: ExpandAsk, page: { filter?: string; offset?: number; count?: number } = {}, signal?: AbortSignal): Promise<StudioExpansion> {
  // `valueSet` is any ValueSet resource: the schema says only that it is an object.
  const body: Omit<StudioExpandInput, 'valueSet'> & { valueSet?: ValueSet } = 'valueSet' in ask ? { valueSet: ask.valueSet, ...page } : { url: ask.url, ...(ask.version === null || ask.version === undefined ? {} : { version: ask.version }), ...page }
  const response = await transport.request({ method: 'POST', path: `${base(id)}/expand`, body, ...withSignal(signal) })
  return parsed<StudioExpansion>('expandStudioValueSet', response.status, response.body)
}
