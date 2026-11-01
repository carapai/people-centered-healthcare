import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// Reception registration and registry review, by keyboard (#71, #203, #228).

async function enterPerson(page: Page, family: string, given: string, sex: 'female' | 'male', nin: string) {
  await type(page, /Family name/, family)
  await type(page, /Given names/, given)
  await page.getByLabel(/^Sex/).selectOption(sex)
  await type(page, 'National ID (NIN)', nin)
  await type(page, 'Age in years (if the date is not known)', '34')
}

const enterGrace = (page: Page) => enterPerson(page, 'Nakato', 'Grace', 'female', 'CF12345678ABCD')

test('a clerk registers, finds and chooses candidates, by keyboard', async ({ page }) => {
  await signIn(page, USERS.clerk)
  await press(page, 'Register a patient', 'link')
  await expect(page.getByRole('heading', { name: 'Register a patient' })).toBeVisible()
  await noViolations(page)

  // A first registration: nobody like her yet.
  await enterGrace(page)
  await press(page, 'Check for this patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toContainText(/KAM013-C1-\d{6}/)
  await press(page, 'Register another patient')

  // The same person again: candidates, chosen with the arrow keys and Enter.
  await enterGrace(page)
  await press(page, 'Check for this patient')
  await expect(page.getByRole('heading', { name: 'Did you mean one of these patients?' })).toBeVisible()
  await noViolations(page)
  const register = page.getByRole('button', { name: 'Register as a new patient' })
  await expect(register).toBeDisabled()
  await page.getByRole('grid').getByRole('columnheader').first().focus()
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('Enter')
  await expect(page.getByRole('status').filter({ hasText: 'Existing patient chosen:' })).toContainText('Grace Nakato')
})

test('a duplicate is created only after the clerk confirms none of the candidates is the patient', async ({ page }) => {
  await signIn(page, USERS.clerk)
  await press(page, 'Register a patient', 'link')
  // Someone to be a candidate, registered first (each test brings its own data).
  await enterPerson(page, 'Okello', 'Peter', 'male', 'CM98765432WXYZ')
  await press(page, 'Check for this patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()
  await press(page, 'Register another patient')
  await enterPerson(page, 'Okello', 'Peter', 'male', 'CM98765432WXYZ')
  await press(page, 'Check for this patient')
  await expect(page.getByRole('heading', { name: 'Did you mean one of these patients?' })).toBeVisible()
  const register = page.getByRole('button', { name: 'Register as a new patient' })
  await expect(register).toBeDisabled()
  await page.getByLabel('I have checked: this patient is none of these').focus()
  await page.keyboard.press('Space')
  await expect(register).toBeEnabled()
  await press(page, 'Register as a new patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toContainText('KAM013-C1-')
})

test('the registry review screen has no accessibility violations', async ({ page }) => {
  await signIn(page, USERS.reviewer)
  await press(page, 'Registry review', 'link')
  await expect(page.getByRole('heading', { name: 'Registry review' })).toBeVisible()
  await noViolations(page)
})
