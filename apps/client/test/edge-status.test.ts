import { createIntl } from 'react-intl'
import { describe, expect, test } from 'vitest'

import { EdgeStatusError, REFRESH_MS, statusQuery, usedPercent } from '../src/features/edge-status/api'
import { formatBytes } from '../src/features/edge-status/StatusPage'
import { parseJson } from '../src/platform/json'
import { fakeTransport } from './fake'

const report = parseJson(
  JSON.stringify({
    at: '2026-11-02T09:00:00Z',
    node: { id: 'facility-edge', version: '0.1.0', started: '2026-11-02T08:00:00Z' },
    store_bytes: 1048576,
    volumes: [{ name: 'data', total_bytes: 100, free_bytes: 15 }],
    backup: null,
    ups: { power: 'on-battery' },
    tasks: [],
    telemetry: { buffered_bytes: 0, dropped_bytes: 0, shipping: false, last_shipped: null, failing: false },
    pending: ['sync'],
    alerts: [{ code: 'disk-full', subject: 'data' }],
  }),
)

describe('edge status', () => {
  test('the report is fetched from the edge, refreshed, with plain numbers', async () => {
    const transport = fakeTransport(() => ({ body: report }))
    const query = statusQuery(transport)
    expect(query.refetchInterval).toBe(REFRESH_MS)
    const got = await query.queryFn({})
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /admin/status'])
    expect(got.store_bytes).toBe(1048576)
    expect(got.volumes[0] === undefined ? undefined : usedPercent(got.volumes[0])).toBe(85)
    expect(got.alerts).toEqual([{ code: 'disk-full', subject: 'data' }])
  })

  test('a refusal carries its status, so the page tells a remote browser from a failure', async () => {
    const transport = fakeTransport(() => ({ status: 403 }))
    await expect(statusQuery(transport).queryFn({})).rejects.toEqual(new EdgeStatusError(403))
  })

  test('sizes are shown in the largest unit that keeps them at least 1', () => {
    const intl = createIntl({ locale: 'en' })
    expect(formatBytes(intl, 512)).toBe('512 byte')
    expect(formatBytes(intl, 1536)).toBe('1.5 kB')
    expect(formatBytes(intl, 3 * 1024 ** 3)).toBe('3 GB')
    expect(usedPercent({ total_bytes: 0, free_bytes: 0 })).toBe(0)
  })
})
