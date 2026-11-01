import { expect, test, type Page } from '@playwright/test'

import { newPage, noViolations, press, signIn, signInByPhone, type } from './helpers'
import { USERS } from './users'

// The patient portal (#111): a patient signs in with a code to their phone and registers
// themselves; a lookalike of a patient already registered at a facility waits for a reviewer,
// who links the account only after checking the patient's ID in person.

async function register(page: Page, given: string, family: string) {
  await expect(page.getByRole('button', { name: 'Register' })).toBeVisible()
  await noViolations(page)
  await type(page, /Given names/, given)
  await type(page, /Family name/, family)
  await page.getByLabel(/Date of birth/).fill('1988-09-21')
  await page.getByLabel(/^Sex/).selectOption('female')
  await press(page, 'Register')
}

test('a new patient signs in with their phone and registers themselves', async ({ page }) => {
  await signInByPhone(page, '0772 100001', '+256772100001', true)
  await register(page, 'Rose', 'Achieng')
  await expect(page.getByRole('status').filter({ hasText: 'You are registered.' })).toBeVisible()
  await noViolations(page)
})

test('a lookalike of an existing patient is linked only after an in-person ID check', async ({ browser }) => {
  // The patient is already registered at reception, with the same phone.
  const reception = await newPage(browser)
  await signIn(reception, USERS.clerk)
  await press(reception, 'Register a patient', 'link')
  await type(reception, /Family name/, 'Auma')
  await type(reception, /Given names/, 'Joyce')
  await reception.getByLabel(/^Sex/).selectOption('female')
  await reception.getByLabel(/Date of birth/).fill('1988-09-21')
  await type(reception, /^Phone/, '0772100002')
  await press(reception, 'Check for this patient')
  await expect(reception.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()
  await reception.context().close()

  // Her portal registration matches: it waits for review, and she sees nothing yet.
  const patient = await newPage(browser)
  await signInByPhone(patient, '0772100002', '+256772100002')
  await register(patient, 'Joyce', 'Auma')
  await expect(patient.getByRole('status').filter({ hasText: 'We are checking' })).toBeVisible()

  // The reviewer opens the newest item, picks the document checked, and links her.
  const reviewer = await newPage(browser)
  await signIn(reviewer, USERS.reviewer)
  await press(reviewer, 'Registry review', 'link')
  await reviewer.getByRole('grid').getByRole('columnheader').first().focus()
  await reviewer.keyboard.press('Control+End')
  await reviewer.keyboard.press('Enter')
  await expect(reviewer.getByRole('heading', { name: 'Portal registration' })).toBeVisible()
  await noViolations(reviewer)
  const link = reviewer.getByRole('button', { name: 'ID checked: this is the patient' }).first()
  await expect(link).toBeDisabled()
  await reviewer.getByLabel('ID document checked in person').selectOption('national-id')
  await expect(link).toBeEnabled()
  await link.focus()
  await reviewer.keyboard.press('Enter')
  await expect(reviewer.getByRole('heading', { name: 'Portal registration' })).toBeHidden()

  await press(patient, 'Check again')
  await expect(patient.getByRole('status').filter({ hasText: 'Your account is linked to your health record.' })).toBeVisible()
})
