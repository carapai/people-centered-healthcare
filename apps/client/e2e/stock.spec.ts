import { expect, test } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// Receiving (#130): a storekeeper receives a delivery, filling the batch and expiry from its
// barcode, and the stock is on hand at its unit cost.

test('a receipt increases the stock on hand, with its cost recorded', async ({ page }) => {
  await signIn(page, USERS.storekeeper)
  await press(page, 'Stock', 'link')
  await expect(page.getByRole('heading', { name: 'Stock' })).toBeVisible()
  await noViolations(page)

  await press(page, 'Receive goods', 'link')
  await type(page, /Delivery or invoice number/, 'INV-2026-0042')
  await type(page, /Barcode/, '(01)09501101530003(17)270630(10)B42')
  await press(page, 'Read barcode')
  await expect(page.getByLabel(/^Batch/)).toHaveValue('B42')
  await expect(page.getByLabel(/^Expiry/)).toHaveValue('2027-06-30')
  await type(page, /^Product/, 'amoxicillin-500')
  await type(page, /Quantity/, '1000')
  await page.getByLabel(/Pack size/).fill('100')
  await type(page, /Unit cost/, '1.50')
  await noViolations(page)
  await press(page, 'Receive')
  await expect(page.getByRole('status').filter({ hasText: 'Received.' })).toBeVisible()

  await press(page, 'Stock', 'link')
  const row = page.getByRole('grid', { name: 'Stock on hand' }).getByRole('row').filter({ hasText: 'amoxicillin-500' })
  await expect(row).toContainText('B42')
  await expect(row).toContainText('1000')
  await expect(row).toContainText('1.50')
  await noViolations(page)
})
