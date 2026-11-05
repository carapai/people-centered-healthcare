import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// #104: a clinician completes a consultation from one page: the patient's summary, a note, a
// diagnosis, a test order and a prescription, then completes the visit. Keyboard only, with axe.

/** Registers a patient with a name unique to this run and checks them in at consultation. */
async function checkedIn(page: Page): Promise<string> {
  const run = String(Date.now()).slice(-8)
  const family = `Okot${[...run].map((d) => String.fromCodePoint(97 + Number(d))).join('')}`
  await signIn(page, USERS.clerk)
  await press(page, 'Register a patient', 'link')
  await type(page, /Family name/, family)
  await type(page, /Given names/, 'Sarah')
  await page.getByLabel(/^Sex/).selectOption('female')
  await type(page, 'National ID (NIN)', `CF${run}CONS`)
  await type(page, 'Age in years (if the date is not known)', '41')
  await press(page, 'Check for this patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()
  // Back home in the app: the session lives in memory, so a reload would end it.
  await page.goBack()
  await press(page, 'Reception', 'link')
  await type(page, 'Family name', family)
  await press(page, 'Find the patient')
  await press(page, `Choose Sarah ${family}`)
  await page.getByLabel('Service point').selectOption('consultation')
  await press(page, 'Check in')
  await expect(page.getByRole('status').filter({ hasText: 'is checked in at consultation.' })).toBeVisible()
  return `Sarah ${family}`
}

test('a clinician completes a consultation without leaving the page', async ({ page, browser, baseURL }) => {
  const name = await checkedIn(page)

  const context = await browser.newContext({ baseURL })
  const clinician = await context.newPage()
  await signIn(clinician, USERS.clinician)
  await press(clinician, 'Consultation', 'link')
  await press(clinician, `See ${name}`)
  await expect(clinician.getByRole('heading', { name: `Consultation: ${name}`, level: 1 })).toBeVisible()
  const conditions = clinician.getByRole('region', { name: 'Conditions' })
  await expect(conditions.getByText('None recorded.')).toBeVisible()
  await noViolations(clinician)

  await type(clinician, 'Note', 'Fever and headache for three days.')
  await press(clinician, 'Save note')
  await expect(clinician.getByRole('status').filter({ hasText: 'Note saved.' })).toBeVisible()

  await type(clinician, 'Diagnosis', 'Malaria')
  await press(clinician, 'Add diagnosis')
  await expect(clinician.getByRole('status').filter({ hasText: 'Diagnosis added: Malaria.' })).toBeVisible()
  await expect(conditions.getByText('Malaria · confirmed')).toBeVisible()

  await type(clinician, 'Test or procedure', 'Malaria RDT')
  await press(clinician, 'Order')
  await expect(clinician.getByRole('status').filter({ hasText: 'Ordered: Malaria RDT.' })).toBeVisible()

  await type(clinician, 'Medicine', 'Artemether/lumefantrine 20/120 mg')
  await type(clinician, 'Dosage', '4 tablets twice daily for 3 days')
  await press(clinician, 'Prescribe')
  await expect(clinician.getByRole('status').filter({ hasText: 'Prescribed: Artemether/lumefantrine 20/120 mg.' })).toBeVisible()
  await expect(clinician.getByRole('region', { name: 'Medications' }).getByText('Artemether/lumefantrine 20/120 mg · 4 tablets twice daily for 3 days')).toBeVisible()
  await noViolations(clinician)

  await press(clinician, 'Complete the visit')
  await expect(clinician.getByRole('status').filter({ hasText: 'The visit is complete.' })).toBeVisible()
  await press(clinician, 'Back to the queue')
  await expect(clinician.getByRole('button', { name: `See ${name}` })).toHaveCount(0)
  await context.close()
})
