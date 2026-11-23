import { expect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { firstVisits, staff, startEdge } from './edge'

// Standard Dashboards on the edge, offline (#879; ADR 0075): the edge has no central to reach.
// The sample Reporting package (e2e/fixtures/dashboards) is active on it with the sample
// indicators, and women had their first ANC visit here today (e2e/edge/edge.ts). A facility user opens the ANC
// overview from the Dashboards tab and sees the figures this node computed for its own facility.

test('a facility user opens a standard Dashboard on the edge, offline, over their facility’s figures', async ({ browser }) => {
  const edge = await startEdge({ signIn: true, dashboards: true })
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
    await devices.getByLabel('Device name').fill('Records PC')
    await devices.getByLabel('Location or ward').fill('Records')
    await devices.getByRole('button', { name: 'Approve a device' }).click()
    await expect(devices.getByRole('status')).toHaveText('Records PC is enrolled.')

    await device.goto('/')
    await device.getByRole('button', { name: 'Sign in' }).click()
    await device.getByLabel('Username').fill(staff.username)
    await device.getByLabel('Password').fill(staff.password)
    await device.getByRole('button', { name: 'Sign in' }).click()
    await expect(device.getByRole('main').getByRole('status')).toHaveText('Signed in')

    await device.getByRole('link', { name: 'Dashboards', exact: true }).click()
    const list = device.getByRole('navigation', { name: 'Dashboards' })
    const overview = list.getByRole('button', { name: /^ANC overview/ })
    await expect(overview).toContainText('From ug.moh.sample-dashboards@1.0.0')
    // Saved Dashboards are central's; nothing is authored here.
    await expect(list).toContainText('Saved Dashboards are kept at central.')
    await expect(device.getByRole('form', { name: 'Save as' })).toHaveCount(0)
    await overview.click()

    // Each area under the facility is the facility itself: one row, this month's figure the
    // first visits recorded here, from the summaries this node computed.
    const card = device.getByRole('article', { name: 'ANC 1st visits by area' })
    const table = card.getByRole('table', { name: 'Figures' })
    await expect(table.getByRole('rowheader')).toHaveCount(1)
    await expect(table.getByRole('row').last().getByRole('cell').last()).toHaveText(String(firstVisits))
    await expect(card).toContainText('Computed on this facility.')
    await expect(device.getByRole('article', { name: 'ANC 1st visits this quarter' })).toContainText(String(firstVisits))
    await noViolations(device)
  } finally {
    await edge.stop()
  }
})
