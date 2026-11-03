import { type Browser, expect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { type Edge, staff, startEdge } from './edge'

// The idle lock and the PIN on the edge (#75; offline-auth.md § Tokens and sessions): after 15
// minutes without use the device locks and hides the user's view; their PIN on this device
// unlocks it, and a wrong PIN does not.

/** An enrolled device signed in as `staff`, with the page's clock under the test's control. */
async function signedIn(browser: Browser, edge: Edge) {
  const admin = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
  expect((await admin.request.post('/api/admin/provision/accounts', { data: edge.bundle ?? '' })).status()).toBe(200)
  const device = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
  await device.clock.install()
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
  await device.goto('/')
  await device.getByRole('button', { name: 'Sign in' }).click()
  await device.getByLabel('Username').fill(staff.username)
  await device.getByLabel('Password').fill(staff.password)
  await device.getByRole('button', { name: 'Sign in' }).click()
  await expect(device.getByRole('main').getByRole('status').first()).toHaveText('Signed in')
  return device
}

test('an idle device locks, and only its user’s PIN on it unlocks it', async ({ browser }) => {
  const edge = await startEdge({ signIn: true })
  try {
    const device = await signedIn(browser, edge)
    await device.getByLabel('New 6-digit PIN').fill('042137')
    await device.getByRole('button', { name: 'Set PIN' }).click()
    await expect(device.getByText('Your PIN is set for this device.')).toBeVisible()

    // Fifteen minutes without use: the view is replaced by the lock screen.
    await device.clock.fastForward('16:00')
    await expect(device.getByRole('heading', { name: 'This device is locked' })).toBeVisible()
    await expect(device.getByText('Signed in', { exact: true })).toBeHidden()
    await noViolations(device)
    // Back to the real time, so the device's DPoP proofs are fresh for the edge.
    await device.clock.setSystemTime(new Date())

    await device.getByLabel('Username').fill(staff.username)
    await device.getByLabel('PIN').fill('999999')
    await device.getByRole('button', { name: 'Unlock' }).click()
    await expect(device.getByRole('alert')).toHaveText('That username and PIN did not unlock this device.')

    await device.getByLabel('PIN').fill('042137')
    await device.getByRole('button', { name: 'Unlock' }).click()
    await expect(device.getByRole('main').getByRole('status').first()).toHaveText('Signed in')
  } finally {
    await edge.stop()
  }
})
