import { expect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { staff, startEdge } from './edge'

// The clinical API on the edge (#106): with central's bundle imported and no internet, staff on
// an enrolled device use the edge's own FHIR API, under the bundle's role mappings and label
// rules. They register a patient, who gets an MRN under the edge's own code, and Reception finds
// them in the edge's store.

test('staff register and find a patient with nothing but the edge', async ({ browser }) => {
  const edge = await startEdge({ signIn: true })
  try {
    const admin = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    // Without an enrolled device's proof, the clinical API is shut.
    expect((await admin.request.get('/api/fhir/Patient')).status()).toBe(401)
    const imported = await admin.request.post('/api/admin/provision/accounts', { data: edge.bundle ?? '' })
    expect(imported.status()).toBe(200)

    const device = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await device.goto('/enroll')
    await device.getByRole('button', { name: 'Ask to enroll' }).click()
    const code = (await device.getByLabel('Pairing code').textContent())?.trim() ?? ''
    await admin.goto('/edge/status')
    const devices = admin.getByRole('region', { name: 'Devices' })
    await devices.getByLabel('Pairing code shown on the device').fill(code)
    await devices.getByLabel('Device name').fill('Reception PC')
    await devices.getByLabel('Location or ward').fill('Reception')
    await devices.getByRole('button', { name: 'Approve a device' }).click()
    await expect(devices.getByRole('status')).toHaveText('Reception PC is enrolled.')

    await device.goto('/')
    await device.getByRole('button', { name: 'Sign in' }).click()
    await device.getByLabel('Username').fill(staff.username)
    await device.getByLabel('Password').fill(staff.password)
    await device.getByRole('button', { name: 'Sign in' }).click()
    await expect(device.getByRole('main').getByRole('status')).toHaveText('Signed in')

    await device.getByRole('link', { name: 'Reception', exact: true }).click()
    await device.getByLabel('Family name').fill('Nobodyhere')
    await device.getByRole('button', { name: 'Find the patient' }).click()
    await expect(device.getByRole('status').filter({ hasText: 'No patient matches. Register them first.' })).toBeVisible()
    await noViolations(device)

    // Registered on the edge: the MRN is minted under the edge's code, E1.
    await device.goBack()
    await device.getByRole('link', { name: 'Register a patient' }).click()
    await device.getByLabel(/Family name/).fill('Achieng')
    await device.getByLabel(/Given names/).fill('Ruth')
    await device.getByLabel(/^Sex/).selectOption('female')
    await device.getByLabel('National ID (NIN)').fill('CF1234567EDGE')
    await device.getByLabel('Age in years (if the date is not known)').fill('31')
    await device.getByRole('button', { name: 'Check for this patient' }).click()
    await expect(device.getByRole('status').filter({ hasText: /Registered\. MRN: .*-E1-\d{6}/ })).toBeVisible()
    await device.goBack()
    await device.getByRole('link', { name: 'Reception', exact: true }).click()
    await device.getByLabel('Family name').fill('Achieng')
    await device.getByRole('button', { name: 'Find the patient' }).click()
    await expect(device.getByRole('button', { name: 'Choose Ruth Achieng' })).toBeVisible()
  } finally {
    await edge.stop()
  }
})
