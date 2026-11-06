import { expect, test, type Browser, type Page } from '@playwright/test'

import { noViolations, press, signIn, type, unique } from './helpers'
import { USERS } from './users'

// #109, #110: a laboratory test from the consultation's order to the clinician's review, through
// the inbox. The result
// reaches the clinician only once someone other than who entered it verifies it, and a critical
// result arrives marked urgent. Keyboard only, with axe on each screen.

/** Registers a patient with a name unique to this run and checks them in at consultation. */
async function checkedIn(page: Page): Promise<string> {
  const { family, digits: run } = unique()
  await signIn(page, USERS.clerk)
  await press(page, 'Register a patient', 'link')
  await type(page, /Family name/, family)
  await type(page, /Given names/, 'Mary')
  await page.getByLabel(/^Sex/).selectOption('female')
  await type(page, 'National ID (NIN)', `CF${run}LABT`)
  await type(page, 'Age in years (if the date is not known)', '28')
  await press(page, 'Check for this patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()
  await page.goBack()
  await press(page, 'Reception', 'link')
  await type(page, 'Family name', family)
  await press(page, 'Find the patient')
  await press(page, `Choose Mary ${family}`)
  await page.getByLabel('Service point').selectOption('consultation')
  await press(page, 'Check in')
  await expect(page.getByRole('status').filter({ hasText: 'is checked in at consultation.' })).toBeVisible()
  return `Mary ${family}`
}

async function as(browser: Browser, baseURL: string | undefined, user: { username: string; password: string }) {
  const context = await browser.newContext({ baseURL })
  const page = await context.newPage()
  await signIn(page, user)
  return { context, page }
}

async function worklist(page: Page, list: string) {
  await page.getByLabel('Worklist').selectOption({ label: list })
}

test('a result reaches the clinician only once a second person verifies it', async ({ page, browser, baseURL }) => {
  // Four people, each signing in: longer than the default.
  test.setTimeout(90_000)
  const name = await checkedIn(page)

  // The clinician orders a haemoglobin.
  const clinician = await as(browser, baseURL, USERS.clinician)
  await press(clinician.page, 'Consultation', 'link')
  await press(clinician.page, `See ${name}`)
  await type(clinician.page, 'Test or procedure', 'Haemoglobin')
  await press(clinician.page, 'Order')
  await expect(clinician.page.getByRole('status').filter({ hasText: 'Ordered: Haemoglobin.' })).toBeVisible()
  // An unsigned note, for the inbox (#110).
  await type(clinician.page, 'Subjective', 'Tired for two weeks.')
  await press(clinician.page, 'Save draft')
  await expect(clinician.page.getByRole('status').filter({ hasText: 'Draft saved.' })).toBeVisible()
  await clinician.page.goBack()
  await press(clinician.page, 'Results to review', 'link')
  await expect(clinician.page.getByText(`Haemoglobin for ${name}`)).toHaveCount(0)

  // The lab collects and results it, and may not verify its own result.
  const tech = await as(browser, baseURL, USERS.labtech)
  await press(tech.page, 'Laboratory', 'link')
  await press(tech.page, `Open Haemoglobin for ${name}`)
  await noViolations(tech.page)
  await press(tech.page, 'Record collection')
  await expect(tech.page.getByRole('status').filter({ hasText: 'Collection recorded.' })).toBeVisible()
  await worklist(tech.page, 'Results to enter')
  await press(tech.page, `Open Haemoglobin for ${name}`)
  await type(tech.page, 'Value', '6.4')
  await type(tech.page, 'Unit', 'g/dL')
  await type(tech.page, 'Reference range, low', '11.5')
  await type(tech.page, 'Reference range, high', '16.5')
  await type(tech.page, 'Critical below', '7.0')
  await press(tech.page, 'Save results')
  await expect(tech.page.getByRole('status').filter({ hasText: 'Results saved; they wait for verification.' })).toBeVisible()
  await worklist(tech.page, 'Results to verify')
  await press(tech.page, `Open Haemoglobin for ${name}`)
  await expect(tech.page.getByText('Haemoglobin: 6.4 g/dL (Critically low)')).toBeVisible()
  await press(tech.page, 'Verify results')
  await expect(tech.page.getByRole('alert').filter({ hasText: 'verified by someone other than who entered them' })).toBeVisible()
  await tech.context.close()

  // Still nothing for the clinician.
  await clinician.page.goBack()
  await press(clinician.page, 'Results to review', 'link')
  await expect(clinician.page.getByText(`Haemoglobin for ${name}`)).toHaveCount(0)

  // A second person verifies it.
  const verifier = await as(browser, baseURL, USERS.verifier)
  await press(verifier.page, 'Laboratory', 'link')
  await worklist(verifier.page, 'Results to verify')
  await press(verifier.page, `Open Haemoglobin for ${name}`)
  await press(verifier.page, 'Verify results')
  await expect(verifier.page.getByRole('status').filter({ hasText: 'Verified and released to the clinician.' })).toBeVisible()
  await verifier.context.close()

  // The inbox links the result and the unsigned note to their records (#110).
  await clinician.page.goBack()
  await press(clinician.page, 'Inbox', 'link')
  const inboxResult = clinician.page.getByRole('listitem').filter({ hasText: `Haemoglobin for ${name}` })
  await expect(inboxResult.getByText('Urgent')).toBeVisible()
  await expect(clinician.page.getByRole('listitem').filter({ hasText: `Progress note for ${name}` }).getByText('Note to finish')).toBeVisible()
  await noViolations(clinician.page)
  await inboxResult.getByRole('link', { name: 'Open Haemoglobin' }).focus()
  await clinician.page.keyboard.press('Enter')
  await expect(clinician.page.getByRole('heading', { name: 'DiagnosticReport', level: 1 })).toBeVisible()
  await expect(clinician.page.getByText('final', { exact: true })).toBeVisible()
  await clinician.page.goBack()
  await clinician.page.goBack()

  // Now the clinician sees it, urgently, and reviews it.
  await press(clinician.page, 'Results to review', 'link')
  const result = clinician.page.getByRole('listitem').filter({ hasText: `Haemoglobin for ${name}` })
  await expect(result.getByText('Urgent: critical result')).toBeVisible()
  await expect(result.getByText('Haemoglobin: 6.4 g/dL (Critically low)')).toBeVisible()
  await noViolations(clinician.page)
  await press(clinician.page, 'Mark Haemoglobin reviewed')
  await expect(clinician.page.getByRole('status').filter({ hasText: 'Marked reviewed.' })).toBeVisible()
  await expect(clinician.page.getByText(`Haemoglobin for ${name}`)).toHaveCount(0)
  await clinician.context.close()
})
