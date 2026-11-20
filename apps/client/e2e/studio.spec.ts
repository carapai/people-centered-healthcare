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
  await expect(page.getByRole('button', { name: 'Submit' })).toHaveAccessibleDescription('You need the metadata.author permission')
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

// #494: the OPD draft's author edits one of its files in the browser. A second tab that opened
// the same file before the first one saved is refused, keeps nothing, and is told to reload.
// *Check now* then shows the checks on the draft page, and the draft stays a draft.
test('the author edits a file, a stale second tab is refused, and Check now keeps the draft a draft', async ({ page, browser }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await expect(page.getByRole('heading', { name: 'Files' })).toBeVisible()
  await page.getByRole('link', { name: 'Open Questionnaire-opd-triage.json' }).click()
  const content = page.getByLabel('Content (JSON)')
  await expect(content).toHaveValue(/"resourceType": "Questionnaire"/)
  await noViolations(page)

  // A second tab, signed in as the same author, opens the same file before the first saves.
  const other = await (await browser.newContext()).newPage()
  await signIn(other, USERS.author)
  await press(other, 'Studio', 'link')
  await other.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await other.getByRole('link', { name: 'Open Questionnaire-opd-triage.json' }).click()
  const otherContent = other.getByLabel('Content (JSON)')
  await expect(otherContent).toHaveValue(/"resourceType": "Questionnaire"/)

  const text = await content.inputValue()
  await content.fill(text.replace('"status": "active"', '"status": "active", "description": "Edited in the first tab"'))
  await press(page, 'Save')
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()

  const otherText = await otherContent.inputValue()
  await otherContent.fill(otherText.replace('"status": "active"', '"status": "active", "description": "Edited in the second tab"'))
  await press(other, 'Save')
  await expect(other.getByRole('alert').filter({ hasText: 'Someone saved this draft after you opened it' })).toBeVisible()
  await press(other, 'Reload')
  await expect(otherContent).toHaveValue(/Edited in the first tab/)
  await other.context().close()

  // Check now runs the checks; the results show on the draft page, and it is still a draft.
  await press(page, 'Back to the package', 'link')
  await expect(page.getByRole('heading', { name: 'Who must act' })).toBeVisible()
  await press(page, 'Check now')
  await expect(page.getByRole('status').filter({ hasText: 'Done.' })).toBeVisible()
  await expect(page.getByText(/^Validation (passed|failed)/)).toBeVisible()
  await expect(page.getByText(/^Simulation (passed|failed)/)).toBeVisible()
  await expect(page.getByRole('button', { name: 'Submit' })).toBeEnabled()
  await noViolations(page)
})

// #493: the OPD draft's holder hands it over to a second author. Until they accept, the holder
// keeps it; once they do, the former holder may no longer edit it. They hand it back after.
test('a handover takes effect only when accepted, and the former holder can no longer edit', async ({ page, browser }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await expect(page.getByRole('heading', { name: 'Who must act' })).toBeVisible()
  await type(page, 'Hand over or reassign to', USERS.coauthor.username)
  await press(page, 'Hand over')
  await expect(page.getByRole('status').filter({ hasText: 'Done.' })).toBeVisible()
  await expect(page.getByText(/who is handing it over to/)).toBeVisible()
  // Not accepted yet: the holder still edits and submits.
  await expect(page.getByRole('button', { name: 'Submit' })).toBeEnabled()
  await noViolations(page)

  const other = await (await browser.newContext()).newPage()
  await signIn(other, USERS.coauthor)
  await press(other, 'Studio', 'link')
  await expect(other.getByRole('grid', { name: 'Studio' }).getByText('Accept the handover')).toBeVisible()
  await other.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await expect(other.getByRole('button', { name: 'Submit' })).toBeDisabled()
  await press(other, 'Accept the handover')
  await expect(other.getByRole('status').filter({ hasText: 'Done.' })).toBeVisible()
  await expect(other.getByRole('button', { name: 'Submit' })).toBeEnabled()
  await noViolations(other)

  // The former holder may no longer edit or submit it.
  await page.reload()
  await page.waitForURL(/\/sign-in\?/)
  await type(page, 'Username', USERS.author.username)
  await page.keyboard.press('Tab')
  await page.keyboard.type(USERS.author.password)
  await page.keyboard.press('Enter')
  await expect(page.getByRole('heading', { name: 'Who must act' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Submit' })).toHaveAccessibleDescription("Only the draft's holder may do that")
  await expect(page.getByText("You cannot change this package: Only the draft's holder may do that")).toBeVisible()

  // Handed back, so the draft is its author's again.
  await type(other, 'Hand over or reassign to', USERS.author.username)
  await press(other, 'Hand over')
  await expect(other.getByRole('status').filter({ hasText: 'Done.' })).toBeVisible()
  await other.context().close()
  await page.reload()
  await page.waitForURL(/\/sign-in\?/)
  await type(page, 'Username', USERS.author.username)
  await page.keyboard.press('Tab')
  await page.keyboard.type(USERS.author.password)
  await page.keyboard.press('Enter')
  await press(page, 'Accept the handover')
  await expect(page.getByRole('button', { name: 'Submit' })).toBeEnabled()
})
