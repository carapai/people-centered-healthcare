import { type QueryClient, queryOptions } from '@tanstack/react-query'

import type { FhirClient, HistoryTarget, ResourceType } from './fhir'
import type { InvalidationHint, Platform, QueryParams } from './types'

/**
 * Query keys (client-application.md § State): per resource type, id and search. Everything for a
 * type sits under `['fhir', type]`, so a type-level hint invalidates it all.
 */
export const fhirKeys = {
  all: ['fhir'] as const,
  type: (type: string) => ['fhir', type] as const,
  resource: (type: string, id: string) => ['fhir', type, 'id', id] as const,
  version: (type: string, id: string, versionId: string) => ['fhir', type, 'id', id, 'version', versionId] as const,
  search: (type: string, params: QueryParams) => ['fhir', type, 'search', params] as const,
  history: (target: HistoryTarget, params: QueryParams = {}) =>
    target.type === undefined
      ? (['fhir', 'history', params] as const)
      : (['fhir', target.type, 'history', 'id' in target ? (target.id ?? null) : null, params] as const),
}

export const worklistKey = (key: string) => ['worklist', key] as const

export function readQuery<T extends ResourceType>(fhir: FhirClient, type: T, id: string) {
  return queryOptions({
    queryKey: fhirKeys.resource(type, id),
    queryFn: ({ signal }) => fhir.read(type, id, { signal }),
  })
}

/** A version never changes, so it is never refetched while cached. */
export function vreadQuery<T extends ResourceType>(fhir: FhirClient, type: T, id: string, versionId: string) {
  return queryOptions({
    queryKey: fhirKeys.version(type, id, versionId),
    queryFn: ({ signal }) => fhir.vread(type, id, versionId, { signal }),
    staleTime: Infinity,
  })
}

export function searchQuery(fhir: FhirClient, type: ResourceType, params: QueryParams, via: 'GET' | 'POST' = 'GET') {
  return queryOptions({
    queryKey: fhirKeys.search(type, params),
    queryFn: ({ signal }) => fhir.search(type, params, { via, signal }),
  })
}

export function historyQuery(fhir: FhirClient, target: HistoryTarget, params: QueryParams = {}) {
  return queryOptions({
    queryKey: fhirKeys.history(target, params),
    queryFn: ({ signal }) => fhir.history(target, params, { signal }),
  })
}

/**
 * Applies one invalidation hint. A changed resource stales its own entry and its type's searches
 * and histories (any of which may now include or exclude it), and system history.
 */
export function invalidate(queryClient: QueryClient, hint: InvalidationHint): Promise<void>[] {
  const pending: Promise<void>[] = []
  if (hint.worklist !== undefined) {
    pending.push(queryClient.invalidateQueries({ queryKey: worklistKey(hint.worklist) }))
  }
  const { resourceType, id } = hint
  if (resourceType !== undefined && id === undefined) {
    pending.push(queryClient.invalidateQueries({ queryKey: fhirKeys.type(resourceType) }))
  } else if (resourceType !== undefined && id !== undefined) {
    pending.push(
      queryClient.invalidateQueries({ queryKey: fhirKeys.resource(resourceType, id), exact: true }),
      queryClient.invalidateQueries({
        predicate: ({ queryKey }) =>
          queryKey[0] === 'fhir' &&
          (queryKey[1] === 'history' || (queryKey[1] === resourceType && (queryKey[2] === 'search' || queryKey[2] === 'history'))),
      }),
    )
  }
  return pending
}

/**
 * Wires the Query cache to the platform: event hints invalidate keys, and the end of a session
 * (logout or lock) clears the cache, so no clinical data outlives it (ADR 0037).
 */
export function connectQueryCache(platform: Pick<Platform, 'events' | 'session'>, queryClient: QueryClient): () => void {
  const unsubscribe = platform.events.subscribe((hint) => void Promise.all(invalidate(queryClient, hint)))
  const offEnd = platform.session.onEnd(() => queryClient.clear())
  return () => {
    unsubscribe()
    offEnd()
  }
}
