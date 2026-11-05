import { expect, test } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// #102: a receptionist checks a registered patient in, and the patient appears in the triage
// queue, where a nurse calls and sees them. Keyboard only, with axe on each screen.
test('a receptionist checks a patient in and the patient appears in the triage queue', async ({ page, browser, baseURL }) => {
  // Unique to this run: the backend's database outlives a run.
  const run = String(Date.now()).slice(-8)
  const family = `Auma${[...run].map((d) => String.fromCodePoint(97 + Number(d))).join('')}`
  const name = `Rose ${family}`
  await signIn(page, USERS.clerk)
  await press(page, 'Register a patient', 'link')
  await type(page, /Family name/, family)
  await type(page, /Given names/, 'Rose')
  await page.getByLabel(/^Sex/).selectOption('female')
  await type(page, 'National ID (NIN)', `CF${run}RCPT`)
  await type(page, 'Age in years (if the date is not known)', '29')
  await press(page, 'Check for this patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()

  // Back home in the app: the session lives in memory, so a reload would end it.
  await page.goBack()
  await press(page, 'Reception', 'link')
  await expect(page.getByRole('heading', { name: 'Reception', level: 1 })).toBeVisible()
  await type(page, 'Family name', family)
  await type(page, 'Given names', 'Rose')
  await press(page, 'Find the patient')
  await press(page, `Choose ${name}`)
  await expect(page.getByRole('status').filter({ hasText: `Patient: ${name}` })).toBeVisible()
  await expect(page.getByLabel('Service point')).toHaveValue('triage')
  await noViolations(page)
  await press(page, 'Check in')
  await expect(page.getByRole('status').filter({ hasText: `${name} is checked in at triage.` })).toBeVisible()

  // Back home in the app: the session lives in memory, so a reload would end it.
  await page.goBack()
  await press(page, 'Queues', 'link')
  const triage = page.getByRole('list', { name: 'triage' })
  await expect(triage.getByText(name, { exact: true })).toBeVisible()
  await expect(triage.getByText('Waiting', { exact: true })).toBeVisible()
  await noViolations(page)

  // A nurse calls and sees her.
  const context = await browser.newContext({ baseURL })
  const nurse = await context.newPage()
  await signIn(nurse, USERS.nurse)
  await press(nurse, 'Queues', 'link')
  await press(nurse, `Call ${name}`)
  await expect(nurse.getByRole('list', { name: 'triage' }).getByText('Called', { exact: true })).toBeVisible()
  await press(nurse, `Start seeing ${name}`)
  await expect(nurse.getByRole('list', { name: 'triage' }).getByText('Being seen', { exact: true })).toBeVisible()
  await context.close()
})
