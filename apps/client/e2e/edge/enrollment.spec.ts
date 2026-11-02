import { expect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { startEdge } from './edge'

// Device enrollment (#74; lan-security.md § Device enrollment; ADR 0009): a device the edge does
// not know cannot call its API; it asks with a key it cannot export and shows a pairing code; the
// admin approves it on the edge's own screen, after which its requests pass; revoked, it is
// refused again.

test('an unknown device is refused, an approved one gets in, a revoked one is shut out', async ({ browser }) => {
  const edge = await startEdge()
  try {
    // No device key, no proof: the API is closed.
    const stranger = await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })
    expect((await stranger.request.get('/api/device')).status()).toBe(401)

    const device = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await device.goto('/')
    await device.getByRole('link', { name: 'Enroll this device' }).click()
    await expect(device.getByRole('heading', { name: 'Enroll this device' })).toBeVisible()
    await noViolations(device)
    await device.getByRole('button', { name: 'Ask to enroll' }).click()
    const code = (await device.getByLabel('Pairing code').textContent())?.trim() ?? ''
    expect(code).toMatch(/^\d{6}$/)
    await expect(device.getByRole('main').getByRole('status')).toHaveText('Waiting for approval…')
    // The key is the browser's own and cannot be read out.
    const extractable = await device.evaluate(
      () =>
        new Promise<boolean>((done, fail) => {
          const open = indexedDB.open('health-device')
          open.addEventListener('error', () => fail(open.error))
          open.addEventListener('success', () => {
            const get = open.result.transaction('keys').objectStore('keys').get('device')
            get.addEventListener('success', () => done((get.result as CryptoKeyPair).privateKey.extractable))
          })
        }),
    )
    expect(extractable).toBe(false)

    // The admin, on the edge's own screen, sees the request without its code and types it.
    const admin = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await admin.goto('/edge/status')
    const devices = admin.getByRole('region', { name: 'Devices' })
    await expect(devices).toContainText(/on .*, until/)
    await expect(devices).not.toContainText(code)
    await devices.getByLabel('Pairing code shown on the device').fill(code)
    await devices.getByLabel('Device name').fill('Ward 3 PC')
    await devices.getByLabel('Location or ward').fill('Ward 3')
    await devices.getByRole('button', { name: 'Approve a device' }).click()
    await expect(devices.getByRole('status')).toHaveText('Ward 3 PC is enrolled.')
    await noViolations(admin)

    // The device notices, and its proofs now pass the gate.
    await expect(device.getByRole('main').getByRole('status')).toHaveText('This device is enrolled as Ward 3 PC, at Ward 3.')

    // Revoked: its next request is refused, and the page says so.
    await devices.getByRole('button', { name: 'Revoke Ward 3 PC' }).click()
    await devices.getByRole('button', { name: 'Yes, revoke Ward 3 PC' }).click()
    await expect(devices).toContainText('revoked')
    await device.reload()
    await expect(device.getByRole('main').getByRole('alert')).toContainText('This device was revoked.')
    // Starting over makes a new key, which can ask again.
    await device.getByRole('button', { name: 'Start over' }).click()
    await device.getByRole('button', { name: 'Ask to enroll' }).click()
    await expect(device.getByLabel('Pairing code')).toHaveText(/^\d{6}$/)
  } finally {
    await edge.stop()
  }
})
