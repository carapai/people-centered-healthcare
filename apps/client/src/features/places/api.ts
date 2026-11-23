import type { Transport } from '../../platform/types'
import type { Place, PlaceLevel } from '../area/api'

export type { Place, PlaceLevel }

/** A request central refused: the status, its explanation, and an NHFR file's problems. */
export class PlacesError extends Error {
  constructor(
    readonly status: number,
    readonly detail: string,
    readonly problems: string[] = [],
  ) {
    super(`the places request failed: ${status} ${detail}`)
    this.name = 'PlacesError'
  }
}

function refused(status: number, body: unknown): PlacesError {
  const fields = typeof body === 'object' && body !== null ? (body as Record<string, unknown>) : {}
  const problems = Array.isArray(fields['problems']) ? fields['problems'].filter((p): p is string => typeof p === 'string') : []
  return new PlacesError(status, typeof fields['detail'] === 'string' ? fields['detail'] : '', problems)
}

function isObject(body: unknown): body is Record<string, unknown> {
  return typeof body === 'object' && body !== null
}

// --- The NHFR import (`POST /api/places/nhfr/preview` and `/apply`, places.manage) ---

/** A facility before and after the import (`health_facility_registry::Change`). */
export interface PlaceChange {
  before: Place
  after: Place
}

/** What importing an NHFR file does (`health_facility_registry::ImportPlan`). */
export interface ImportPlan {
  /** Regions and districts of the national core package not in the registry yet. */
  hierarchy: Place[]
  new: Place[]
  changed: PlaceChange[]
  /** Facilities the file lists as closed: they become inactive, and are kept. */
  closed: PlaceChange[]
  unchanged: number
  /** Codes of facilities in the registry the file does not list: left alone. */
  absent: string[]
}

/** What an import wrote. */
export interface Applied {
  written: number
  plan: ImportPlan
}

async function nhfr<T>(transport: Transport, step: 'preview' | 'apply', csv: string): Promise<T> {
  const response = await transport.request({ method: 'POST', path: `/places/nhfr/${step}`, body: { csv } })
  if (response.status !== 200 || !isObject(response.body)) throw refused(response.status, response.body)
  return response.body as T
}

/** What importing the NHFR export `csv` would do; nothing is written. */
export function previewImport(transport: Transport, csv: string): Promise<ImportPlan> {
  return nhfr<ImportPlan>(transport, 'preview', csv)
}

/** Imports the NHFR export `csv`, in one transaction. */
export function applyImport(transport: Transport, csv: string): Promise<Applied> {
  return nhfr<Applied>(transport, 'apply', csv)
}

/** Whether a plan changes nothing. */
export function planIsEmpty(plan: ImportPlan): boolean {
  return plan.hierarchy.length === 0 && plan.new.length === 0 && plan.changed.length === 0 && plan.closed.length === 0
}

/** Where a place is, as text, to compare. */
function where(place: Place): string {
  return place.coordinates === undefined ? '' : `${place.coordinates.latitude},${place.coordinates.longitude}`
}

/** The fields of a facility that an import changes, by name: what the review shows. */
export function changedFields(change: PlaceChange): ('name' | 'parent' | 'active' | 'care_level' | 'ownership' | 'coordinates')[] {
  const { before, after } = change
  const fields = ['name', 'parent', 'active', 'care_level', 'ownership'] as const
  const changed: ('name' | 'parent' | 'active' | 'care_level' | 'ownership' | 'coordinates')[] = fields.filter((f) => (before[f] ?? null) !== (after[f] ?? null))
  if (where(before) !== where(after)) changed.push('coordinates')
  return changed
}

// --- Role assignment at a place (`/api/assignments`, access.administer) ---

/** The roles the access policy defines, and the places of the administrator's area. */
export interface AssignmentOptions {
  roles: string[]
  /** The session's place first, then the places below it by code. */
  places: Place[]
}

/** Roles a user holds at one place. */
export interface Assignment {
  place: Place
  roles: string[]
  /** When the permission snapshot expires. */
  not_after: string
  /** Whether the place lies in the administrator's area. */
  manageable: boolean
}

/** A user's places and roles. */
export interface UserAssignments {
  username: string
  account: string
  assignments: Assignment[]
}

/** The query keys of role assignment, under one root so a change refreshes them all. */
export const ASSIGNMENTS = 'assignments'

/** The roles and places an administrator may assign. */
export function optionsQuery(transport: Transport) {
  return {
    queryKey: [ASSIGNMENTS, 'options'],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<AssignmentOptions> => {
      const response = await transport.request({ method: 'GET', path: '/assignments/options', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || !isObject(response.body)) throw refused(response.status, response.body)
      return response.body as unknown as AssignmentOptions
    },
  }
}

/** The places and roles of `username`. */
export function userQuery(transport: Transport, username: string) {
  return {
    queryKey: [ASSIGNMENTS, 'user', username],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<UserAssignments> => {
      const response = await transport.request({ method: 'GET', path: `/assignments/users/${encodeURIComponent(username)}`, ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || !isObject(response.body)) throw refused(response.status, response.body)
      return response.body as unknown as UserAssignments
    },
  }
}

/** Replaces `username`'s roles at `place` with `roles`. */
export async function assign(transport: Transport, username: string, place: string, roles: string[]): Promise<void> {
  const response = await transport.request({ method: 'PUT', path: `/assignments/users/${encodeURIComponent(username)}/${encodeURIComponent(place)}`, body: { roles } })
  if (response.status !== 200) throw refused(response.status, response.body)
}

/** Withdraws every role `username` holds at `place`. */
export async function withdraw(transport: Transport, username: string, place: string): Promise<void> {
  const response = await transport.request({ method: 'DELETE', path: `/assignments/users/${encodeURIComponent(username)}/${encodeURIComponent(place)}` })
  if (response.status !== 204) throw refused(response.status, response.body)
}

/** A place's label for a list: its name, and its level unless it is a facility. */
export function placeLabel(place: Place, levelName: (level: PlaceLevel) => string): string {
  return place.level === 'facility' ? `${place.name} (${place.code})` : `${place.name} (${levelName(place.level)})`
}
