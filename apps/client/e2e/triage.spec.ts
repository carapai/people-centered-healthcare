import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn, type, unique } from './helpers'
import { USERS } from './users'

// #103: a nurse triages a patient checked in at triage with the OPD triage form (ug.moh.opd).
// The node extracts each vital sign as an Observation; the patient then waits for consultation,
// where the clinician's summary shows them. Keyboard only, with axe on the form.

/** Registers a patient with a name unique to this run and checks them in at triage. */
async function atTriage(page: Page): Promise<string> {
  const { family, digits: run } = unique()
  await signIn(page, USERS.clerk)
  await press(page, 'Register a patient', 'link')
  await type(page, /Family name/, family)
  await type(page, /Given names/, 'Grace')
  await page.getByLabel(/^Sex/).selectOption('female')
  await type(page, 'National ID (NIN)', `CF${run}TRGE`)
  await type(page, 'Age in years (if the date is not known)', '34')
  await press(page, 'Check for this patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()
  // Back home in the app: the session lives in memory, so a reload would end it.
  await page.goBack()
  await press(page, 'Reception', 'link')
  await type(page, 'Family name', family)
  await press(page, 'Find the patient')
  await press(page, `Choose Grace ${family}`)
  await expect(page.getByLabel('Service point')).toHaveValue('triage')
  await press(page, 'Check in')
  await expect(page.getByRole('status').filter({ hasText: 'is checked in at triage.' })).toBeVisible()
  return `Grace ${family}`
}

test('a nurse records vital signs at triage and the clinician sees them', async ({ page, browser, baseURL }) => {
  test.setTimeout(90_000)
  const name = await atTriage(page)

  const context = await browser.newContext({ baseURL })
  const nurse = await context.newPage()
  await signIn(nurse, USERS.nurse)
  await press(nurse, 'Triage', 'link')
  await press(nurse, `Triage ${name}`)
  await expect(nurse.getByRole('heading', { name: `Triage: ${name}`, level: 1 })).toBeVisible()
  await nurse.getByLabel(/^Temperature/).fill('39.2')
  await nurse.getByLabel(/^Pulse/).fill('118')
  await nurse.getByLabel(/^Respiratory rate/).fill('26')
  await nurse.getByLabel(/^Oxygen saturation/).fill('94')
  await nurse.getByLabel(/^Weight/).fill('61.5')
  await nurse.getByLabel(/^Systolic/).fill('104')
  await nurse.getByLabel(/^Diastolic/).fill('66')
  await nurse.getByRole('radiogroup', { name: /Triage category/ }).getByLabel('Urgent: seen next').check()
  await noViolations(nurse)
  await press(nurse, 'Submit')
  await expect(nurse.getByRole('status').filter({ hasText: `${name} is triaged and waiting for consultation.` })).toBeVisible()
  await expect(nurse.getByRole('button', { name: `Triage ${name}` })).toHaveCount(0)

  // The clinician sees the extracted vital signs in the patient's summary.
  const clinicianContext = await browser.newContext({ baseURL })
  const clinician = await clinicianContext.newPage()
  await signIn(clinician, USERS.clinician)
  await press(clinician, 'Consultation', 'link')
  await press(clinician, `See ${name}`)
  const vitals = clinician.getByRole('region', { name: 'Vital signs' })
  await expect(vitals.getByText('Body temperature: 39.2 °C')).toBeVisible()
  await expect(vitals.getByText('Heart rate: 118 beats/min')).toBeVisible()
  await expect(vitals.getByText('Body weight: 61.5 kg')).toBeVisible()
  await expect(vitals.getByText('Oxygen saturation in Arterial blood by Pulse oximetry: 94 %')).toBeVisible()
  // Blood pressure is one panel, read from its components.
  await expect(vitals.getByText('Blood pressure panel with all children optional: 104/66 mmHg')).toBeVisible()
})
