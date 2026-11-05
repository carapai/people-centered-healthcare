import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #92: a nurse fills the ANC visit form. Its live behaviour runs in the browser (fhirpath.js), a
// draft is saved on the server and continued, and the response is submitted. Keyboard only, with
// axe on the form.
test('a form is filled with its live behaviour, saved as a draft, continued and submitted', async ({ page }) => {
  await signIn(page, USERS.nurse)
  await press(page, 'Forms', 'link')
  await expect(page.getByRole('heading', { name: 'Forms', level: 1 })).toBeVisible()
  await page.getByLabel('Form', { exact: true }).selectOption({ label: 'ANC visit' })
  await press(page, 'Start a new response')
  await expect(page.getByRole('heading', { name: 'ANC visit' })).toBeVisible()

  // enableWhenExpression: counselling is asked of adolescents only.
  await expect(page.getByText('Adolescent counselling given')).toHaveCount(0)
  await page.getByLabel(/^Age/).fill('16')
  await expect(page.getByText('Adolescent counselling given')).toBeVisible()
  await page.getByLabel(/^Age/).fill('25')
  await expect(page.getByText('Adolescent counselling given')).toHaveCount(0)

  // enableWhen: the last period is asked once she is pregnant, and it is required.
  await expect(page.getByLabel(/^Last menstrual period/)).toHaveCount(0)
  await page.getByRole('radiogroup', { name: /Pregnant/ }).getByLabel('Yes').check()
  await expect(page.getByLabel(/^Last menstrual period/)).toBeVisible()
  await noViolations(page)
  await press(page, 'Submit')
  await expect(page.getByRole('alert').filter({ hasText: '1 required question is not answered.' })).toBeVisible()

  // A draft on the server, continued.
  await press(page, 'Save draft')
  await expect(page.getByRole('status').filter({ hasText: 'Draft saved.' })).toBeVisible()
  await page.getByLabel('Form', { exact: true }).selectOption({ label: 'Choose' })
  await page.getByLabel('Form', { exact: true }).selectOption({ label: 'ANC visit' })
  await press(page, /Continue the draft saved/)
  await expect(page.getByLabel(/^Age/)).toHaveValue('25')

  await page.getByLabel(/^Last menstrual period/).fill('2026-07-01')
  await press(page, 'Submit')
  await expect(page.getByRole('status').filter({ hasText: 'Submitted.' })).toBeVisible()
})
