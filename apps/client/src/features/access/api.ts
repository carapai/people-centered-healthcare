import type { Transport } from '../../platform/types'

/** A role granted at this edge offline, until central ratifies it (`health-auth` § local_admin). */
export interface LocalGrant {
  id: string
  account: string
  /** The user's username, where the edge knows it. */
  username: string | null
  role: string
  granted_by: string
  granted_at: string
  not_after: string
}

/** Local access revoked at this edge. */
export interface LocalRevocation {
  account: string
  username: string | null
  revoked_by: string
  revoked_at: string
}

/** A provisional account just created for new staff. */
export interface ProvisionalAccount {
  account: string
  username: string
  role: string
  not_after: string
}

/** A request the edge refused: the status, and its error code (`refused`, `no-such-user`, …) and detail. */
export class AccessError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    readonly detail: string,
  ) {
    super(`access request failed: ${status} ${code}`)
    this.name = 'AccessError'
  }
}

/** The query keys of the page's lists, under one root so a change refreshes them all. */
export const ACCESS = 'local-access'

function refused(status: number, body: unknown): AccessError {
  const fields = typeof body === 'object' && body !== null ? (body as Record<string, unknown>) : {}
  return new AccessError(status, typeof fields['error'] === 'string' ? fields['error'] : 'failed', typeof fields['detail'] === 'string' ? fields['detail'] : '')
}

async function list<T>(transport: Transport, path: string, signal?: AbortSignal): Promise<T[]> {
  const response = await transport.request({ method: 'GET', path, ...(signal === undefined ? {} : { signal }) })
  if (response.status !== 200 || !Array.isArray(response.body)) throw refused(response.status, response.body)
  return response.body as T[]
}

async function create<T>(transport: Transport, path: string, form: Record<string, string>): Promise<T> {
  const response = await transport.request({ method: 'POST', path, form })
  if (response.status !== 201) throw refused(response.status, response.body)
  return response.body as T
}

/** The roles this edge may grant offline. */
export function rolesQuery(transport: Transport) {
  return { queryKey: [ACCESS, 'roles'], queryFn: ({ signal }: { signal?: AbortSignal }) => list<string>(transport, '/access/roles', signal) }
}

/** The facility's unexpired local grants. */
export function grantsQuery(transport: Transport) {
  return { queryKey: [ACCESS, 'grants'], queryFn: ({ signal }: { signal?: AbortSignal }) => list<LocalGrant>(transport, '/access/grants', signal) }
}

/** The local revocations. */
export function revocationsQuery(transport: Transport) {
  return { queryKey: [ACCESS, 'revocations'], queryFn: ({ signal }: { signal?: AbortSignal }) => list<LocalRevocation>(transport, '/access/revocations', signal) }
}

/** Grants `role` to `username` for `days` (1 to 7). */
export function grant(transport: Transport, username: string, role: string, days: number): Promise<LocalGrant> {
  return create(transport, '/access/grants', { username, role, days: String(days) })
}

/** Revokes `username`'s access here at once. */
export function revoke(transport: Transport, username: string): Promise<LocalRevocation> {
  return create(transport, '/access/revocations', { username })
}

/** A provisional account for new staff: `password` is temporary, and its user must change it at first sign-in. */
export function provision(transport: Transport, account: { username: string; password: string; role: string; days: number }): Promise<ProvisionalAccount> {
  return create(transport, '/access/accounts', { username: account.username, password: account.password, role: account.role, days: String(account.days) })
}
