import { expect, test, type Browser, type Page } from '@playwright/test'

import { noViolations } from '../helpers'
import { staff, startEdge, type Edge } from './edge'

// The §132 acceptance (#106; tests/acceptance/opd-lan): several browsers on one edge, with no
// internet, run the OPD visit end to end: registration and check-in at reception, triage with
// vital signs extracted on the edge, and the consultation that sees them and completes the
// visit. Each browser is an enrolled device; the edge is all they reach.

// A missing control fails the step quickly instead of waiting out the whole test.
test.use({ actionTimeout: 30_000 })

async function enrolled(browser: Browser, edge: Edge, admin: Page, name: string): Promise<Page> {
  const device = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
  await device.goto('/enroll')
  await device.getByRole('button', { name: 'Ask to enroll' }).click()
  const code = (await device.getByLabel('Pairing code').textContent())?.trim() ?? ''
  await admin.goto('/edge/status')
  const devices = admin.getByRole('region', { name: 'Devices' })
  await devices.getByLabel('Pairing code shown on the device').fill(code)
  await devices.getByLabel('Device name').fill(name)
  await devices.getByLabel('Location or ward').fill(name)
  await devices.getByRole('button', { name: 'Approve a device' }).click()
  await expect(devices.getByRole('status')).toHaveText(`${name} is enrolled.`)
  await device.goto('/')
  await device.getByRole('button', { name: 'Sign in' }).click()
  await device.getByLabel('Username').fill(staff.username)
  await device.getByLabel('Password').fill(staff.password)
  await device.getByRole('button', { name: 'Sign in' }).click()
  await expect(device.getByRole('main').getByRole('status')).toHaveText('Signed in')
  return device
}

test('an OPD visit runs end to end on the edge with the internet down', async ({ browser }) => {
  test.setTimeout(600_000)
  const edge = await startEdge({ signIn: true, opd: true })
  try {
    const admin = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    expect((await admin.request.post('/api/admin/provision/accounts', { data: edge.bundle ?? '' })).status()).toBe(200)
    const reception = await enrolled(browser, edge, admin, 'Reception PC')
    const triage = await enrolled(browser, edge, admin, 'Triage PC')
    const consultation = await enrolled(browser, edge, admin, 'Consultation PC')

    // Reception registers the patient (an MRN under the edge's code) and checks her in.
    await reception.getByRole('link', { name: 'Register a patient' }).click()
    await reception.getByLabel(/Family name/).fill('Namutebi')
    await reception.getByLabel(/Given names/).fill('Esther')
    await reception.getByLabel(/^Sex/).selectOption('female')
    await reception.getByLabel('National ID (NIN)').fill('CF9876543OPDL')
    await reception.getByLabel('Age in years (if the date is not known)').fill('27')
    await reception.getByRole('button', { name: 'Check for this patient' }).click()
    await expect(reception.getByRole('status').filter({ hasText: /Registered\. MRN: KLA001-E1-\d{6}/ })).toBeVisible()
    await reception.goBack()
    await reception.getByRole('link', { name: 'Reception', exact: true }).click()
    await reception.getByLabel('Family name').fill('Namutebi')
    await reception.getByRole('button', { name: 'Find the patient' }).click()
    await reception.getByRole('button', { name: 'Choose Esther Namutebi' }).click()
    await expect(reception.getByLabel('Service point')).toHaveValue('triage')
    await reception.getByRole('button', { name: 'Check in' }).click()
    await expect(reception.getByRole('status').filter({ hasText: 'Esther Namutebi is checked in at triage.' })).toBeVisible()

    // Triage, on another device: vital signs and an urgent category, extracted on the edge.
    await triage.getByRole('link', { name: 'Triage', exact: true }).click()
    await triage.getByRole('button', { name: 'Triage Esther Namutebi' }).click()
    await triage.getByLabel(/^Temperature/).fill('38.6')
    await triage.getByLabel(/^Pulse/).fill('104')
    await triage.getByLabel(/^Systolic/).fill('112')
    await triage.getByLabel(/^Diastolic/).fill('70')
    await triage.getByRole('radiogroup', { name: /Triage category/ }).getByLabel('Urgent: seen next').check()
    await triage.getByRole('button', { name: 'Submit' }).click()
    await expect(triage.getByRole('status').filter({ hasText: 'Esther Namutebi is triaged and waiting for consultation.' })).toBeVisible()

    // The consultation, on a third: the edge's summary shows what triage recorded.
    await consultation.getByRole('link', { name: 'Consultation', exact: true }).click()
    await consultation.getByRole('button', { name: 'See Esther Namutebi' }).click()
    const vitals = consultation.getByRole('region', { name: 'Vital signs' })
    await expect(vitals.getByText('Body temperature: 38.6 °C')).toBeVisible()
    await expect(vitals.getByText('Heart rate: 104 beats/min')).toBeVisible()
    await expect(vitals.getByText('Blood pressure panel with all children optional: 112/70 mmHg')).toBeVisible()
    await noViolations(consultation)
    await consultation.getByRole('button', { name: 'Complete the visit' }).click()
    await expect(consultation.getByRole('status').filter({ hasText: 'The visit is complete.' })).toBeVisible()
  } finally {
    await edge.stop()
  }
})
