import { resolve } from 'node:path'

import { expect, test } from '@playwright/test'

import { newPage, noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// #873: administering places at central (ADR 0094), as the national administrator (e2e-national,
// whose `access` role gives places.manage and access.administer at the national place).

test('the national administrator reviews an NHFR file and imports it', async ({ page }) => {
  await signIn(page, USERS.national)
  await press(page, 'Import facilities from the NHFR', 'link')
  await expect(page.getByRole('heading', { name: 'Import facilities from the NHFR' })).toBeVisible()
  // The backend imported the NHFR sample; this export adds Lacor Hospital in Gulu.
  await page.getByLabel('NHFR export (CSV)').setInputFiles(resolve(import.meta.dirname, 'fixtures/nhfr-update.csv'))
  await press(page, 'Review the import')
  const review = page.getByRole('region', { name: 'Review the import' })
  await expect(review.getByRole('heading', { name: 'New facilities (1)' })).toBeVisible()
  await expect(review.getByText('Lacor Hospital (GUL002)')).toBeVisible()
  await expect(review.getByText('7 facilities are unchanged.')).toBeVisible()
  await noViolations(page)
  await press(page, /^Import$/)
  await expect(page.getByRole('status').filter({ hasText: 'Imported' })).toHaveText(/Imported: \d+ places? written\./)

  // The same file again changes nothing.
  await press(page, 'Review the import')
  await expect(page.getByRole('status').filter({ hasText: 'nothing to import' })).toHaveText('The registry already matches this file. There is nothing to import.')
  await expect(page.getByRole('button', { name: 'Import', exact: true })).toHaveCount(0)
})

test('the national administrator assigns a role at a district, and its user signs in for it', async ({ page, browser }) => {
  await signIn(page, USERS.national)
  await press(page, 'Roles at places', 'link')
  await type(page, 'Username', USERS.wakiso.username)
  await press(page, 'Show roles')
  const held = page.getByRole('region', { name: `Roles ${USERS.wakiso.username} holds` })
  await expect(held.getByRole('listitem')).toHaveCount(1)
  await expect(held.getByRole('listitem')).toContainText('Wakiso (district): district')

  const form = page.getByRole('form', { name: 'Assign roles at a place' })
  await form.getByLabel('Place').selectOption({ label: 'Kampala (district)' })
  await form.getByRole('checkbox', { name: 'district', exact: true }).check()
  await noViolations(page)
  await press(page, 'Assign')
  await expect(form.getByRole('status')).toContainText(`${USERS.wakiso.username} now holds district at Kampala (district)`)
  await expect(held.getByRole('listitem')).toHaveCount(2)

  // The user now works at two places: sign-in asks which, and they sign in for Kampala.
  const user = await newPage(browser)
  await user.goto('/')
  await press(user, 'Sign in')
  await user.waitForURL(/\/sign-in\?/)
  await type(user, 'Username', USERS.wakiso.username)
  await type(user, 'Password', USERS.wakiso.password)
  await user.keyboard.press('Enter')
  await user.waitForURL(/error=facility/)
  await type(user, 'Username', USERS.wakiso.username)
  await type(user, 'Password', USERS.wakiso.password)
  await type(user, /Facility code/, 'district-kampala')
  await user.keyboard.press('Enter')
  await expect(user.getByRole('status').filter({ hasText: 'Signed in' })).toBeVisible()
  await press(user, 'My area', 'link')
  await expect(user.getByText('You are signed in for Kampala (district).')).toBeVisible()
  await user.context().close()

  // Withdrawn again, so the user works at Wakiso alone as before.
  const kampala = held.getByRole('listitem').filter({ hasText: 'Kampala (district)' })
  await kampala.getByRole('button', { name: 'Withdraw' }).click()
  await kampala.getByRole('button', { name: 'Confirm: withdraw every role at Kampala' }).click()
  await expect(held.getByRole('listitem')).toHaveCount(1)
})
