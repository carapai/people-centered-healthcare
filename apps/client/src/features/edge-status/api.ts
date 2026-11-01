import { isLosslessNumber } from '../../platform/json'
import type { Transport } from '../../platform/types'

/** The edge's status report (`health_edge_runtime::admin::Report`). It holds no personal health data. */
export interface EdgeReport {
  at: string
  node: { id: string; version: string; started: string }
  store_bytes: number | null
  volumes: { name: 'data' | 'backup'; total_bytes: number; free_bytes: number }[]
  backup: {
    interval_seconds: number
    last_backup: string | null
    failed: string | null
    last_test: string | null
    test_passed: boolean | null
  } | null
  ups: { power: 'online' | 'on-battery' | 'low-battery' | 'unreachable' | null } | null
  tasks: { name: string; recent_failures: number }[]
  telemetry: {
    buffered_bytes: number
    dropped_bytes: number
    shipping: boolean
    last_shipped: string | null
    failing: boolean
  }
  pending: string[]
  alerts: { code: string; subject: string | null }[]
}

/** A report the edge refused or could not give: the status, for the page to explain. */
export class EdgeStatusError extends Error {
  constructor(readonly status: number) {
    super(`edge status failed: ${status}`)
    this.name = 'EdgeStatusError'
  }
}

/** How often the page refreshes the report. */
export const REFRESH_MS = 30_000

/** The edge's status report, refreshed every {@link REFRESH_MS}. */
export function statusQuery(transport: Transport) {
  return {
    queryKey: ['edge-status'],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<EdgeReport> => {
      const response = await transport.request({ method: 'GET', path: '/admin/status', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new EdgeStatusError(response.status)
      return plain(response.body) as EdgeReport
    },
    refetchInterval: REFRESH_MS,
  }
}

/** `value` with the transport's lossless numbers as plain numbers: the report's are byte counts and seconds. */
function plain(value: unknown): unknown {
  if (isLosslessNumber(value)) return Number(value.toString())
  if (Array.isArray(value)) return value.map(plain)
  if (typeof value === 'object' && value !== null) return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, plain(v)]))
  return value
}

/** A volume's share used, in whole percent. */
export function usedPercent(volume: { total_bytes: number; free_bytes: number }): number {
  if (volume.total_bytes === 0) return 0
  return Math.floor(((volume.total_bytes - Math.min(volume.free_bytes, volume.total_bytes)) * 100) / volume.total_bytes)
}
