import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn, type, unique } from './helpers'
import { USERS } from './users'

// #102: a receptionist checks a registered patient in, and the patient appears in the triage
// queue, where a nurse calls and sees them. Keyboard only, with axe on each screen.
test('a receptionist checks a patient in and the patient appears in the triage queue', async ({ page, browser, baseURL }) => {
  // Unique to this run: the backend's database outlives a run.
  const { family, digits: run } = unique()
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
  // Earlier runs leave their patients in the queue, so look only at this one's entry.
  const entry = (p: Page) => p.getByRole('list', { name: 'triage' }).getByRole('listitem').filter({ hasText: name })
  await expect(entry(page).getByText(name, { exact: true })).toBeVisible()
  await expect(entry(page).getByText('Waiting', { exact: true })).toBeVisible()
  await noViolations(page)

  // A nurse calls and sees her.
  const context = await browser.newContext({ baseURL })
  const nurse = await context.newPage()
  await signIn(nurse, USERS.nurse)
  await press(nurse, 'Queues', 'link')
  await press(nurse, `Call ${name}`)
  await expect(entry(nurse).getByText('Called', { exact: true })).toBeVisible()
  await press(nurse, `Start seeing ${name}`)
  await expect(entry(nurse).getByText('Being seen', { exact: true })).toBeVisible()
  await context.close()
})
