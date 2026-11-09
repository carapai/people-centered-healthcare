import type { Transport } from '../../platform/types'

// The Conflict Record review queue (#82; synchronization.md § Conflict Record and review queue):
// `/api/sync/conflicts` at central, for staff whose roles grant `sync.resolve-conflicts`. The
// server keeps both branches and audits every decision; nothing here merges anything.

/** One conflicting version of a record, as the queue lists it. */
export interface ConflictItem {
  op_id: string
  /** `Type/id`. */
  resource: string
  parent: string | null
  head: string | null
  class: string
}

/** An open Conflict Record. */
export interface ConflictSummary {
  id: string
  /** The node whose push opened it. */
  peer: string
  class: string
  policy: string
  status: string
  openedAt: string
  overdue: boolean
  items: ConflictItem[]
}

/** A pushed version: the candidate branch. */
export interface Candidate {
  resource_type: string
  resource_id: string
  op_id: string
  body: Record<string, unknown>
}

/** One Conflict Record with both branches: the candidates as pushed, and each record's head here. */
export interface ConflictDetail extends ConflictSummary {
  candidates: Candidate[]
  heads: Record<string, Record<string, unknown> | null>
}

/** How a record is resolved: the head kept, or a version written on top of it. */
export type Decision = { decision: 'keep-head'; resource: string } | { decision: 'write'; resource: string; json: Record<string, unknown> }

/** A request the server refused, with its status. */
export class ConflictError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string = '',
  ) {
    super(`conflict review failed: ${status}`)
    this.name = 'ConflictError'
  }
}

export const conflictsKey = ['sync', 'conflicts'] as const

async function get<T>(transport: Transport, path: string): Promise<T> {
  const response = await transport.request({ method: 'GET', path })
  if (response.status !== 200) {
    const body = response.body as { detail?: unknown } | undefined
    throw new ConflictError(response.status, typeof body?.detail === 'string' ? body.detail : '')
  }
  return response.body as T
}

/** The open records, oldest first. */
export function conflictsQuery(transport: Transport) {
  return {
    queryKey: conflictsKey,
    queryFn: async () => (await get<{ conflicts: ConflictSummary[] }>(transport, '/sync/conflicts')).conflicts,
  }
}

/** One record, with both branches. */
export function conflictQuery(transport: Transport, id: string) {
  return {
    queryKey: [...conflictsKey, id] as const,
    queryFn: () => get<ConflictDetail>(transport, `/sync/conflicts/${encodeURIComponent(id)}`),
  }
}

/** Resolves record `id` as decided. */
export async function resolve(transport: Transport, id: string, decision: Decision): Promise<void> {
  const response = await transport.request({ method: 'POST', path: `/sync/conflicts/${encodeURIComponent(id)}`, body: decision })
  if (response.status !== 200) {
    const body = response.body as { detail?: unknown } | undefined
    throw new ConflictError(response.status, typeof body?.detail === 'string' ? body.detail : '')
  }
}

/** The candidate for `resource` in `detail`, if it pushed one. */
export function candidateFor(detail: ConflictDetail, resource: string): Candidate | undefined {
  return detail.candidates.find((c) => `${c.resource_type}/${c.resource_id}` === resource)
}

/** `resource` without `meta`, which each node stamps: what a reviewer compares and writes. */
export function content(resource: Record<string, unknown> | null | undefined): Record<string, unknown> | undefined {
  if (resource === null || resource === undefined) return undefined
  const { meta: _meta, ...rest } = resource
  return rest
}
