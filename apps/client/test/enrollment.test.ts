import { describe, expect, test } from 'vitest'

import {
  EnrollmentError,
  approve,
  deviceLabel,
  devices,
  enrollmentStatus,
  pendingRequests,
  requestEnrollment,
  revoke,
  thisDevice,
} from '../src/features/enrollment/api'
import { deviceKeys, memoryKeyStorage } from '../src/platform/device'
import { parseJson } from '../src/platform/json'
import { fakeTransport } from './fake'

const device = {
  id: '0190a000-0000-7000-8000-000000000001',
  name: 'Ward 3 tablet',
  location: 'Ward 3',
  label: 'Chrome on Android',
  enrolled_by: 'edge-console',
  enrolled_at: '2026-11-02T09:00:00Z',
  status: 'active',
  revoked_by: null,
  revoked_at: null,
}

const json = (value: unknown) => parseJson(JSON.stringify(value))

describe('device enrollment', () => {
  test('a device asks with its label and gets the code to show', async () => {
    const transport = fakeTransport(() => ({ status: 201, body: json({ request: 'r', code: '042137', expires: '2026-11-02T09:10:00Z' }) }))
    expect(await requestEnrollment(transport, 'Chrome on Windows')).toEqual({ code: '042137', expires: '2026-11-02T09:10:00Z' })
    expect(transport.sent).toEqual([expect.objectContaining({ method: 'POST', path: '/enrollment', body: { label: 'Chrome on Windows' } })])
  })

  test('a refusal carries the edge’s code', async () => {
    const revoked = fakeTransport(() => ({ status: 403, body: json({ error: 'device-revoked', detail: 'x' }) }))
    await expect(requestEnrollment(revoked, 'x')).rejects.toEqual(new EnrollmentError(403, 'device-revoked'))
    const unenrolled = fakeTransport(() => ({ status: 403, body: json({ error: 'device-not-enrolled' }) }))
    await expect(thisDevice(unenrolled)).rejects.toEqual(new EnrollmentError(403, 'device-not-enrolled'))
  })

  test('where the device stands: none, pending, enrolled or revoked', async () => {
    expect(await enrollmentStatus(fakeTransport(() => ({ status: 404, body: json({ status: 'none' }) })))).toEqual({ status: 'none' })
    const pending = { status: 'pending', code: '042137', expires: '2026-11-02T09:10:00Z' }
    expect(await enrollmentStatus(fakeTransport(() => ({ body: json(pending) })))).toEqual(pending)
    expect(await enrollmentStatus(fakeTransport(() => ({ body: json({ status: 'enrolled', device }) })))).toEqual({ status: 'enrolled', device })
    expect(await enrollmentStatus(fakeTransport(() => ({ body: json({ status: 'revoked' }) })))).toEqual({ status: 'revoked' })
  })

  test('the admin lists, approves by code and revokes', async () => {
    const transport = fakeTransport((request) => {
      if (request.path === '/admin/enrollments') return { body: json([{ id: 'r', label: 'Chrome on Android', requested_at: 'a', expires: 'b' }]) }
      if (request.path === '/admin/enrollments/approve') return { status: 201, body: json(device) }
      if (request.path === '/admin/devices') return { body: json([device]) }
      return { body: json({ ...device, status: 'revoked' }) }
    })
    expect(await pendingRequests(transport)).toHaveLength(1)
    expect((await approve(transport, { code: '042137', name: 'Ward 3 tablet', location: 'Ward 3' })).name).toBe('Ward 3 tablet')
    expect(await devices(transport)).toEqual([device])
    expect((await revoke(transport, device.id)).status).toBe('revoked')
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual([
      'GET /admin/enrollments',
      'POST /admin/enrollments/approve',
      'GET /admin/devices',
      `POST /admin/devices/${device.id}/revoke`,
    ])
    const gone = fakeTransport(() => ({ status: 410, body: json({ error: 'expired' }) }))
    await expect(approve(gone, { code: '1', name: 'a', location: 'b' })).rejects.toEqual(new EnrollmentError(410, 'expired'))
  })

  test('the label names the browser and the system', () => {
    const chromeWindows = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36'
    const edge = `${chromeWindows} Edg/131.0`
    const firefoxLinux = 'Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0'
    const chromeAndroid = 'Mozilla/5.0 (Linux; Android 14; SM-T220) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36'
    expect(deviceLabel(chromeWindows)).toBe('Chrome on Windows')
    expect(deviceLabel(edge)).toBe('Edge on Windows')
    expect(deviceLabel(firefoxLinux)).toBe('Firefox on Linux')
    expect(deviceLabel(chromeAndroid)).toBe('Chrome on Android')
    expect(deviceLabel('')).toBe('Browser on an unknown system')
  })
})

describe('device keys', () => {
  test('a key pair is made once, kept, and forgotten on request; its private key never leaves', async () => {
    const storage = memoryKeyStorage()
    const keys = await deviceKeys(storage)
    expect(keys.current()).toBeUndefined()
    const made = await keys.create()
    expect(made.privateKey.extractable).toBe(false)
    await expect(crypto.subtle.exportKey('jwk', made.privateKey)).rejects.toThrow()
    expect(keys.current()).toBe(made)
    // The next start finds it.
    expect((await deviceKeys(storage)).current()).toBe(made)
    await keys.forget()
    expect(keys.current()).toBeUndefined()
    expect((await deviceKeys(storage)).current()).toBeUndefined()
  })
})
