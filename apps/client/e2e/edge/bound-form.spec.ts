import { expect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { staff, startEdge } from './edge'

// Forms in use offer a bound question's codes on the edge, offline (#843): the edge has no
// central to reach. A sample package (e2e/fixtures/bound-form) is active on it, whose form binds
// its main complaint to a ValueSet of three codes (and a retired fourth) and its home district to
// one of fourteen. The edge expands both from the package's terminology.

test('a form bound to ValueSets offers their codes on the edge, offline', async ({ browser }) => {
  const edge = await startEdge({ signIn: true, boundForm: true })
  try {
    const admin = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    expect((await admin.request.post('/api/admin/provision/accounts', { data: edge.bundle ?? '' })).status()).toBe(200)

    const device = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await device.goto('/enroll')
    await device.getByRole('button', { name: 'Ask to enroll' }).click()
    const code = (await device.getByLabel('Pairing code').textContent())?.trim() ?? ''
    await admin.goto('/edge/status')
    const devices = admin.getByRole('region', { name: 'Devices' })
    await devices.getByLabel('Pairing code shown on the device').fill(code)
    await devices.getByLabel('Device name').fill('OPD PC')
    await devices.getByLabel('Location or ward').fill('OPD')
    await devices.getByRole('button', { name: 'Approve a device' }).click()
    await expect(devices.getByRole('status')).toHaveText('OPD PC is enrolled.')

    await device.goto('/')
    await device.getByRole('button', { name: 'Sign in' }).click()
    await device.getByLabel('Username').fill(staff.username)
    await device.getByLabel('Password').fill(staff.password)
    await device.getByRole('button', { name: 'Sign in' }).click()
    await expect(device.getByRole('main').getByRole('status')).toHaveText('Signed in')

    await device.getByRole('link', { name: 'Forms', exact: true }).click()
    await device.getByLabel('Form', { exact: true }).selectOption({ label: 'Presenting complaint' })
    await device.getByRole('button', { name: 'Start a new response' }).click()
    await expect(device.getByRole('heading', { name: 'Presenting complaint' })).toBeVisible()

    // Three codes: radio buttons, the retired one left out.
    const complaint = device.getByRole('radiogroup', { name: /Main complaint/ })
    await expect(complaint.getByRole('radio')).toHaveCount(3)
    await expect(complaint.getByLabel('Malaise')).toHaveCount(0)
    await complaint.getByLabel('Cough').check()

    // Fourteen codes: a drop-down, as there is no search widget yet.
    const district = device.getByLabel(/^Home district/)
    await expect(district.locator('option')).toHaveCount(15)
    await district.selectOption({ label: 'Gulu' })
    await expect(device.getByRole('alert')).toHaveCount(0)
    await noViolations(device)

    await device.getByRole('button', { name: 'Submit' }).click()
    await expect(device.getByRole('status').filter({ hasText: 'Submitted.' })).toBeVisible()
  } finally {
    await edge.stop()
  }
})
