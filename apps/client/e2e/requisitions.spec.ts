import { expect, test } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// Ward requisitions (#250): the ward asks the pharmacy for stock; the pharmacy approves, picks a
// batch and sends it, which takes it off the pharmacy's stock on hand.

test('a ward requisition is approved and sent from the pharmacy', async ({ page }) => {
  await signIn(page, USERS.storekeeper)
  // The pharmacy has stock.
  // The session is in memory: move through the app's links, never reload.
  await press(page, 'Stock', 'link')
  await press(page, 'Receive goods', 'link')
  await type(page, /Delivery or invoice number/, 'INV-REQ-1')
  await type(page, /^Product/, 'ors-sachet')
  await type(page, /^Batch/, 'R1')
  await page.getByLabel(/^Expiry/).fill('2028-01-31')
  await type(page, /Quantity/, '500')
  await type(page, /Unit cost/, '0.40')
  await press(page, 'Receive')
  await expect(page.getByRole('status').filter({ hasText: 'Received.' })).toBeVisible()

  // The ward asks for 120.
  await press(page, 'Stock', 'link')
  await press(page, 'Requisitions', 'link')
  await expect(page.getByRole('heading', { name: 'Requisitions', level: 1 })).toBeVisible()
  await noViolations(page)
  await type(page, /^Product/, 'ors-sachet')
  await type(page, /Quantity/, '120')
  await press(page, 'Send requisition')
  const asked = page.getByRole('listitem').filter({ hasText: 'ors-sachet × 120' })
  await expect(asked).toContainText('Waiting for approval')

  // The pharmacy approves, picks the batch and sends it.
  await page.getByLabel('Your store').fill('Location/pharmacy')
  await expect(asked).toBeVisible()
  await asked.getByRole('button', { name: 'Approve' }).focus()
  await page.keyboard.press('Enter')
  await expect(asked).toContainText('Approved')
  await asked.getByLabel(/Batch to send for/).selectOption('R1')
  await noViolations(page)
  await asked.getByRole('button', { name: 'Send the stock' }).focus()
  await page.keyboard.press('Enter')
  await expect(asked).toContainText('Sent')

  await page.goBack()
  const row = page.getByRole('grid', { name: 'Stock on hand' }).getByRole('row').filter({ hasText: 'ors-sachet' })
  await expect(row).toContainText('380')
})
