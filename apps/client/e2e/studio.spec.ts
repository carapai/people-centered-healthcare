import { expect, test } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// #492: a reviewer opens the Studio at central, finds the OPD draft waiting for its author, and
// reads it top to bottom. Every action is the server's, and one they may not take is disabled
// with its reason. Keyboard only, with axe on both pages.
test('the package list and a draft read from top to bottom, with refused actions and their reasons', async ({ page }) => {
  await signIn(page, USERS.studio)
  await press(page, 'Studio', 'link')
  await expect(page.getByRole('heading', { name: 'Studio', level: 1 })).toBeVisible()
  const grid = page.getByRole('grid', { name: 'Studio' })
  await expect(grid.getByText('ug.moh.opd', { exact: false })).toBeVisible()
  await expect(grid.getByText('Nothing for you')).toBeVisible()
  await noViolations(page)

  await grid.getByRole('columnheader').first().focus()
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('Enter')
  await expect(page.getByRole('heading', { name: 'What changed' })).toBeVisible()
  await expect(page.getByRole('heading', { name: /Is it safe\?/ })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Who must act' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Discussion' })).toBeVisible()
  // In Draft, only its author may submit; approval waits for review.
  await expect(page.getByRole('button', { name: 'Submit' })).toBeDisabled()
  await expect(page.getByText('You need the metadata.author permission')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Approve' })).toBeDisabled()
  await expect(page.getByText('Not possible while it is at the draft stage').first()).toBeVisible()
  await noViolations(page)
})

// #835, #829: a click opens a package. Tokens live in memory only, so a reload signs the person
// out; the page's 401 sends them to sign in, and back to the same package after.
test('a click opens a package, and after a reload sign-in comes back to it', async ({ page }) => {
  await signIn(page, USERS.studio)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await expect(page.getByRole('heading', { name: 'What changed' })).toBeVisible()
  const opened = page.url()
  expect(opened).toMatch(/\/studio\/.+/)

  await page.reload()
  await page.waitForURL(/\/sign-in\?/)
  await type(page, 'Username', USERS.studio.username)
  await page.keyboard.press('Tab')
  await page.keyboard.type(USERS.studio.password)
  await page.keyboard.press('Enter')
  await page.waitForURL(opened)
  await expect(page.getByRole('heading', { name: 'What changed' })).toBeVisible()
})
