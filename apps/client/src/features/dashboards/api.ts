import type { PlaceLevel, Visualization } from '../explorer/api'
import type { Transport } from '../../platform/types'

/** Who a saved Dashboard is shared with (`health_analytics::Share`): a person, a role, or every session at a place level. */
export type Share = { with: 'person'; username: string; account?: string } | { with: 'role'; role: string } | { with: 'level'; level: PlaceLevel }

/** A saved Visualization: a title over the explorer's setup. */
export interface SavedVisualization {
  id: string
  title: string
  visualization: Visualization
}

/** A Dashboard as the session reads it: saved (user content) or standard (from the Reporting package, #603). */
export interface Dashboard {
  id: string
  title: string
  kind: 'saved' | 'standard'
  /** Whether the session made it: only then may it change it, and read its shares. Never a standard one. */
  mine: boolean
  /** A standard Dashboard's Reporting package version, `name@version`. */
  package?: string
  description?: string
  shares?: Share[]
  items: SavedVisualization[]
}

/** What the session may open (`GET /api/analytics/dashboards`). */
export interface Dashboards {
  /** Whether the session holds `analytics.author`: it may save Visualizations and make Dashboards. */
  canAuthor: boolean
  dashboards: Dashboard[]
  /** The session's own Visualizations on none of its Dashboards. */
  visualizations: SavedVisualization[]
}

/** A save or a change was refused: the status and why, for the page to say. */
export class DashboardsError extends Error {
  constructor(
    readonly status: number,
    readonly detail: string | undefined,
  ) {
    super(`the dashboards answered ${status}`)
    this.name = 'DashboardsError'
  }
}

function detailOf(body: unknown): string | undefined {
  return typeof body === 'object' && body !== null && 'detail' in body && typeof body.detail === 'string' ? body.detail : undefined
}

/** The query key every save refreshes. */
export const DASHBOARDS_KEY = ['analytics', 'dashboards'] as const

/** The Dashboards the session may open, and whether it may author. */
export function dashboardsQuery(transport: Transport) {
  return {
    queryKey: DASHBOARDS_KEY,
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Dashboards> => {
      const response = await transport.request({ method: 'GET', path: '/analytics/dashboards', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new DashboardsError(response.status, detailOf(response.body))
      const body = response.body as Dashboards
      return { canAuthor: body.canAuthor === true, dashboards: body.dashboards ?? [], visualizations: body.visualizations ?? [] }
    },
  }
}

async function send<T>(transport: Transport, method: 'POST' | 'PUT', path: string, body: unknown, ok: number): Promise<T> {
  const response = await transport.request({ method, path, body })
  if (response.status !== ok) throw new DashboardsError(response.status, detailOf(response.body))
  return response.body as T
}

/** Saves `visualization` with `title`, onto one of the session's own Dashboards if `dashboard` is given. */
export function saveVisualization(transport: Transport, save: { title: string; visualization: Visualization; dashboard?: string }) {
  return send<SavedVisualization & { dashboard: string | null }>(transport, 'POST', '/analytics/visualizations', save, 201)
}

/** Makes a Dashboard with `title`, shared with `shares`; with `copyOf`, a copy of that standard Dashboard's Visualizations (*Save as*). */
export function createDashboard(transport: Transport, dashboard: { title: string; shares: Share[]; copyOf?: string }) {
  return send<Dashboard>(transport, 'POST', '/analytics/dashboards', dashboard, 201)
}

/** Changes the title and shares of the session's own Dashboard `id`. */
export function changeDashboard(transport: Transport, id: string, dashboard: { title: string; shares: Share[] }) {
  return send<Dashboard>(transport, 'PUT', `/analytics/dashboards/${encodeURIComponent(id)}`, dashboard, 200)
}

/** A share as it is sent: a person by username alone. */
export function sendable(share: Share): Share {
  return share.with === 'person' ? { with: 'person', username: share.username } : share
}

/** The `/explore` search that opens `visualization` in the explorer (*Open in explorer*). */
export function explorerSearch(visualization: Visualization): { v: string } {
  return { v: JSON.stringify(visualization) }
}

const TYPES = ['column', 'line', 'table', 'single', 'map']
const LAYOUTS = ['placesByIndicators', 'indicatorsByPeriods']
const SERIES = ['indicator', 'place', 'band']
const PERIOD_TYPES = ['weekly', 'monthly', 'quarterly', 'financialJuly']
const FOLLOW = ['myPlace', 'eachAreaUnder']
const texts = (v: unknown): v is string[] => Array.isArray(v) && v.every((x) => typeof x === 'string')

/** The Visualization a `/explore?v=` search opens, if it reads as one; anything else opens the explorer as usual. */
export function readExplorerSearch(v: unknown): Visualization | undefined {
  if (typeof v !== 'string') return undefined
  let parsed: unknown
  try {
    parsed = JSON.parse(v)
  } catch {
    return undefined
  }
  if (typeof parsed !== 'object' || parsed === null) return undefined
  const s = parsed as Record<string, unknown>
  const q = s['question'] as Record<string, unknown> | null | undefined
  if (!TYPES.includes(s['type'] as string) || !LAYOUTS.includes(s['layout'] as string) || !SERIES.includes(s['series'] as string)) return undefined
  if (typeof q !== 'object' || q === null || !texts(q['indicators']) || !texts(q['places']) || !PERIOD_TYPES.includes(q['periodType'] as string) || typeof q['from'] !== 'string' || typeof q['to'] !== 'string') return undefined
  if (s['follow'] !== undefined && !FOLLOW.includes(s['follow'] as string)) return undefined
  if (q['disaggregation'] !== undefined && typeof q['disaggregation'] !== 'string') return undefined
  return parsed as Visualization
}

/** The roles or usernames in a comma-separated list, each once. */
export function splitList(text: string): string[] {
  return [...new Set(text.split(',').map((t) => t.trim()).filter((t) => t !== ''))]
}
