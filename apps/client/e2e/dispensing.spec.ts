import { expect, test, type Page } from '@playwright/test'

import { newPage, noViolations, press, signIn, type, unique } from './helpers'
import { USERS } from './users'

// #128 (ADR 0035): stock moves only when a dispense is completed. A prescription and a prepared
// dispense leave stock on hand as it was; handing it over moves it. Keyboard only, with axe.

/** The on-hand row of `product` in the pharmacy store, freshly loaded from the home page. */
async function onHand(page: Page, product: string) {
  // Back home in the app: the session lives in memory, so a reload would end it.
  for (let step = 0; step < 6 && new URL(page.url()).pathname !== '/'; step++) await page.goBack()
  await press(page, 'Stock', 'link')
  return page.getByRole('grid', { name: 'Stock on hand' }).getByRole('row').filter({ hasText: product })
}

test('stock moves only when the dispense is handed over', async ({ page, browser }) => {
  test.setTimeout(120_000)
  const { family, digits } = unique()
  const product = `al-20-120-${family.toLowerCase()}`
  const medicine = 'Artemether/lumefantrine 20/120 mg'

  // The storekeeper receives 48 of a product unique to this run.
  const store = await newPage(browser)
  await signIn(store, USERS.storekeeper)
  await press(store, 'Stock', 'link')
  await press(store, 'Receive goods', 'link')
  await type(store, /Delivery or invoice number/, `INV-${digits}`)
  await type(store, /^Batch/, `AL${digits.slice(0, 4)}`)
  await store.getByLabel(/^Expiry/).fill('2027-06-30')
  await type(store, /^Product/, product)
  await type(store, /Quantity/, '48')
  await store.getByLabel(/Pack size/).fill('12')
  await type(store, /Unit cost/, '0.75')
  await press(store, 'Receive')
  await expect(store.getByRole('status').filter({ hasText: 'Received.' })).toBeVisible()
  await expect(await onHand(store, product)).toContainText('48')

  // A patient is checked in, and the clinician prescribes.
  await signIn(page, USERS.clerk)
  await press(page, 'Register a patient', 'link')
  await type(page, /Family name/, family)
  await type(page, /Given names/, 'Esther')
  await page.getByLabel(/^Sex/).selectOption('female')
  await type(page, 'National ID (NIN)', `CF${digits}DISP`)
  await type(page, 'Age in years (if the date is not known)', '33')
  await press(page, 'Check for this patient')
  await expect(page.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()
  await page.goBack()
  await press(page, 'Reception', 'link')
  await type(page, 'Family name', family)
  await press(page, 'Find the patient')
  await press(page, `Choose Esther ${family}`)
  await page.getByLabel('Service point').selectOption('consultation')
  await press(page, 'Check in')
  await expect(page.getByRole('status').filter({ hasText: 'is checked in at consultation.' })).toBeVisible()
  const name = `Esther ${family}`
  const clinician = await newPage(browser)
  await signIn(clinician, USERS.clinician)
  await press(clinician, 'Consultation', 'link')
  await press(clinician, `See ${name}`)
  await type(clinician, 'Medicine', medicine)
  await type(clinician, 'Dosage', '4 tablets twice daily for 3 days')
  await press(clinician, 'Prescribe')
  await expect(clinician.getByRole('status').filter({ hasText: `Prescribed: ${medicine}.` })).toBeVisible()

  // The pharmacist prepares 24: the stock is reserved, and nothing moves.
  const pharmacist = await newPage(browser)
  await signIn(pharmacist, USERS.pharmacist)
  await press(pharmacist, 'Pharmacy', 'link')
  const entry = pharmacist.getByRole('list', { name: 'Prescriptions to dispense' }).getByRole('listitem').filter({ hasText: name })
  const panel = entry.getByRole('region', { name: `Dispense ${medicine} for ${name}` })
  await panel.getByLabel('Stock product').fill(product)
  await panel.getByLabel('Quantity (base units)').fill('24')
  await noViolations(pharmacist)
  await panel.getByRole('button', { name: 'Prepare' }).click()
  await expect(panel.getByRole('status')).toContainText('nothing has moved yet')
  await expect(panel.getByText(/Batch AL\d{4}, expires 2027-06-30: 24/)).toBeVisible()
  await noViolations(pharmacist)
  await expect(await onHand(store, product)).toContainText('48')

  // Handed over: now the stock is dispensed.
  await panel.getByRole('button', { name: 'Hand over' }).click()
  await expect(panel.getByRole('status')).toContainText('Handed over.')
  const row = await onHand(store, product)
  await expect(row).not.toContainText('48')
  await expect(row).toContainText('24')
})
