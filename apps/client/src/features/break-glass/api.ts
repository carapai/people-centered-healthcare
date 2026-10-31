import { worklistKey } from '../../platform'
import type { Transport } from '../../platform/types'

/** A break-glass grant as the server reports it (`health-auth` § Break-glass). */
export interface Grant {
  id: string
  account: string
  facility: string
  patient: string
  reason: string
  justification: string
  at: string
  expires_at: string
  review_due: string
  acknowledged: boolean
  escalated_at?: string | null
  /** Only in the review queue: whether the review is past due. */
  overdue?: boolean
}

/** The worklists the server's invalidation hints name, so an open list refreshes at once. */
export const ALERTS = 'break-glass-alerts'
export const REVIEWS = 'break-glass-reviews'

/** A request the server refused: the status, for the page to explain. */
export class BreakGlassError extends Error {
  constructor(readonly status: number) {
    super(`break-glass request failed: ${status}`)
    this.name = 'BreakGlassError'
  }
}

async function list(transport: Transport, path: string, signal?: AbortSignal): Promise<Grant[]> {
  const response = await transport.request({ method: 'GET', path, ...(signal === undefined ? {} : { signal }) })
  if (response.status !== 200 || !Array.isArray(response.body)) throw new BreakGlassError(response.status)
  return response.body as Grant[]
}

/** The facility in-charge's unacknowledged alerts. */
export function alertsQuery(transport: Transport) {
  return {
    queryKey: worklistKey(ALERTS),
    queryFn: ({ signal }: { signal?: AbortSignal }) => list(transport, '/break-glass/alerts', signal),
  }
}

/** The privacy officer's pending reviews, the earliest due first. */
export function reviewsQuery(transport: Transport) {
  return {
    queryKey: worklistKey(REVIEWS),
    queryFn: ({ signal }: { signal?: AbortSignal }) => list(transport, '/break-glass/reviews', signal),
  }
}

async function post(transport: Transport, path: string, body?: unknown): Promise<void> {
  const response = await transport.request({ method: 'POST', path, ...(body === undefined ? {} : { body }) })
  if (response.status !== 204) throw new BreakGlassError(response.status)
}

/** The in-charge has seen alert `id`. */
export function acknowledge(transport: Transport, id: string): Promise<void> {
  return post(transport, `/break-glass/alerts/${encodeURIComponent(id)}/acknowledge`)
}

/** The privacy officer's decision on grant `id`; an inappropriate access needs a note. */
export function decide(transport: Transport, id: string, appropriate: boolean, note: string): Promise<void> {
  return post(transport, `/break-glass/reviews/${encodeURIComponent(id)}`, {
    decision: appropriate ? 'appropriate' : 'inappropriate',
    note,
  })
}
