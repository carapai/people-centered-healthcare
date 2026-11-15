import { expect, test } from '@playwright/test'

import { noViolations, press, signIn, type, unique } from './helpers'
import { USERS } from './users'

// Equipment (ADR 0064; #453): a product tracked as equipment is received by serial number, one
// record per unit, never as stock; the unit is marked faulty with a note, moved, and sent to
// another facility, after which this facility no longer holds it.

test('equipment is received by serial, changed, and sent to another facility', async ({ page }) => {
  test.setTimeout(120_000)
  const { family, digits } = unique()
  const product = `bp-machine-${family.toLowerCase()}`
  const serial = `BP-${digits}`
  await signIn(page, USERS.storekeeper)
  await press(page, 'Stock', 'link')

  await press(page, 'Products', 'link')
  await type(page, 'Product code', product)
  await type(page, /^Name$/, `BP machine ${family}`)
  await type(page, /Counted in/, 'unit')
  await type(page, 'Pack size', '1')
  await page.getByLabel('Tracked as equipment').check()
  await noViolations(page)
  await press(page, 'Save product')
  await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible()

  // Back to Stock in the app (a reload would sign out).
  await page.goBack()
  await press(page, 'Receive goods', 'link')
  await type(page, /Delivery or invoice number/, `INV-${digits}`)
  await type(page, /^Product/, product)
  await type(page, /Quantity/, '1')
  await type(page, /Unit cost/, '250000')
  await type(page, /Serial numbers/, serial)
  await noViolations(page)
  await press(page, 'Receive')
  await expect(page.getByRole('status').filter({ hasText: 'Received.' })).toBeVisible()

  await press(page, 'Stock', 'link')
  await press(page, 'Equipment', 'link')
  const row = page.getByRole('grid', { name: 'Equipment held' }).getByRole('row').filter({ hasText: serial })
  await expect(row).toContainText('Available')
  await expect(row).toContainText('Location/pharmacy')
  // Its cost, and with no useful life set its book value, are what it cost.
  await expect(row).toContainText('250000')
  await expect(page.getByText('Book value of the equipment held:')).toBeVisible()
  await noViolations(page)

  await page.getByLabel('Unit to change').selectOption({ label: `BP machine ${family}, serial number ${serial}` })
  await page.getByLabel('New status').selectOption('faulty')
  await type(page, 'What was done or found', 'cuff leaks')
  await press(page, 'Save status')
  await expect(row).toContainText('Faulty')
  await expect(row).toContainText('cuff leaks')

  await type(page, 'Move to location', 'Location/workshop')
  await press(page, 'Move')
  await expect(row).toContainText('Location/workshop')
  await noViolations(page)

  await type(page, /Send to facility/, 'GUL02')
  await press(page, 'Send')
  await expect(page.getByRole('status').filter({ hasText: `Saved BP machine ${family} ${serial}.` })).toBeVisible()
  await expect(page.getByRole('grid', { name: 'Equipment held' }).getByRole('row').filter({ hasText: serial })).toHaveCount(0)
})
