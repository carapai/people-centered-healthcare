import { expect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { staff, startEdge } from './edge'

const SIGN_IN_FAILED = 'Sign-in could not be completed. Please sign in again.'

// Staff sign-in on the edge (#75; offline-auth.md): with central's bundle imported on the edge's
// own screen and no internet, staff sign in on an enrolled device, with tokens bound to its key;
// an unenrolled device gets no token even with the right password.

test('staff sign in on an enrolled device with nothing but the edge', async ({ browser }) => {
  const edge = await startEdge({ signIn: true })
  try {
    // The edge's own screen imports central's bundle.
    const admin = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    const imported = await admin.request.post('/api/admin/provision/accounts', { data: edge.bundle ?? '' })
    expect(imported.status()).toBe(200)
    expect(await imported.json()).toMatchObject({ accounts: 1, snapshots: 1 })

    const signIn = async (page: typeof admin) => {
      await page.goto('/')
      await page.getByRole('button', { name: 'Sign in' }).click()
      await page.getByLabel('Username').fill(staff.username)
      await page.getByLabel('Password').fill(staff.password)
      await page.getByRole('button', { name: 'Sign in' }).click()
    }

    // Not enrolled: the right password, but no token for this device's key.
    const stranger = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await stranger.goto('/enroll')
    await stranger.getByRole('button', { name: 'Ask to enroll' }).click()
    await expect(stranger.getByLabel('Pairing code')).toHaveText(/^\d{6}$/)
    await signIn(stranger)
    // The callback's token exchange is refused: the device's key is not enrolled.
    await expect(stranger).toHaveURL(/\/callback\?/)
    await expect(stranger.getByRole('main').getByRole('status')).toHaveText(SIGN_IN_FAILED)

    // Enrolled and approved, the same credentials sign in.
    const device = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await device.goto('/enroll')
    await device.getByRole('button', { name: 'Ask to enroll' }).click()
    const code = (await device.getByLabel('Pairing code').textContent())?.trim() ?? ''
    await admin.goto('/edge/status')
    const devices = admin.getByRole('region', { name: 'Devices' })
    await devices.getByLabel('Pairing code shown on the device').fill(code)
    await devices.getByLabel('Device name').fill('Ward 3 PC')
    await devices.getByLabel('Location or ward').fill('Ward 3')
    await devices.getByRole('button', { name: 'Approve a device' }).click()
    await expect(devices.getByRole('status')).toHaveText('Ward 3 PC is enrolled.')

    await signIn(device)
    await expect(device.getByRole('main').getByRole('status')).toHaveText('Signed in')
    await noViolations(device)
  } finally {
    await edge.stop()
  }
})
