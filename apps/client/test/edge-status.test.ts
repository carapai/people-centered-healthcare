import { createIntl } from 'react-intl'
import { describe, expect, test } from 'vitest'

import { EdgeStatusError, PairingUnavailable, REFRESH_MS, pairingQuery, statusQuery, usedPercent } from '../src/features/edge-status/api'
import { qrPath } from '../src/features/edge-status/qr'
import { alertText, formatBytes } from '../src/features/edge-status/StatusPage'
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
    encryption: {
      checked: '2026-11-02T09:00:00Z',
      tpm2: true,
      secure_boot: true,
      volumes: [{ name: 'data', protection: 'unencrypted', reason: null }],
      findings: [{ code: 'disk-unencrypted', subject: 'data' }],
      exemption: null,
    },
    certificate: {
      state: 'serving',
      names: ['facility-edge.local'],
      addresses: ['192.168.1.10'],
      expires: '2027-12-04T09:00:00Z',
      renews: '2027-08-01T09:00:00Z',
      intermediate_expires: '2031-11-02T09:00:00Z',
      problem: null,
      address_changed: null,
    },
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
    expect(got.certificate.state).toBe('serving')
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

  test('every encryption alert the edge raises has its sentence', () => {
    const intl = createIntl({ locale: 'en', defaultLocale: 'en' })
    const codes = [
      ['disk-unencrypted', 'system'],
      ['tpm-protector-missing', 'data'],
      ['encryption-suspended', 'data'],
      ['tpm-not-sealed-to-secure-boot', 'data'],
      ['encryption-unknown', 'data'],
      ['tpm-missing', null],
      ['tpm-unknown', null],
      ['secure-boot-off', null],
      ['secure-boot-unknown', null],
      ['encryption-exempt', null],
    ] as const
    for (const [code, subject] of codes) {
      const text = alertText(intl, { code, subject })
      expect(text).not.toBe(code)
      if (subject !== null) expect(text).toContain(`${subject} disk`)
    }
  })

  test('every certificate alert the edge raises has its sentence', () => {
    const intl = createIntl({ locale: 'en', defaultLocale: 'en' })
    for (const code of ['not-provisioned', 'identity-invalid', 'certificate-renewal-failed', 'certificate-expiring', 'intermediate-expiring', 'address-changed']) {
      expect(alertText(intl, { code, subject: null })).not.toBe(code)
    }
  })

  test('the pairing payload comes from the edge, and its absence says why', async () => {
    const pairing = { version: '1', node_id: 'ug-kla-0001', url: 'https://ug-kla-0001.local', spki: 'sha256/abc=', uri: 'health-edge://pair?v=1&node=ug-kla-0001' }
    const transport = fakeTransport(() => ({ body: pairing }))
    expect(await pairingQuery(transport).queryFn({})).toEqual(pairing)
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /admin/pairing'])
    const refused = fakeTransport(() => ({ status: 409, body: { error: 'not-provisioned', detail: 'x' } }))
    await expect(pairingQuery(refused).queryFn({})).rejects.toEqual(new PairingUnavailable('not-provisioned'))
    await expect(pairingQuery(fakeTransport(() => ({ status: 403 }))).queryFn({})).rejects.toEqual(new EdgeStatusError(403))
  })

  test('the QR code is a square of dark modules inside a quiet zone', () => {
    const { size, path } = qrPath('health-edge://pair?v=1&node=ug-kla-0001&url=https%3A%2F%2Fug-kla-0001.local')
    // Version 3 or 4 at level M, plus 4 modules of quiet zone on each side.
    expect(size).toBeGreaterThanOrEqual(29 + 8)
    expect(path.startsWith('M4 4h1v1h-1z')).toBe(true)
    expect(path).not.toMatch(/M[0-3] |M\d+ [0-3]h/)
  })
})
