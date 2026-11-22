import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #537: the indicator editor. The ANC reference package (ug.moh.anc) is the author's draft, with
// its indicators and test patients. Try evaluates the unsaved definition on central over them.

test('an indicator is tried over the test patients, its denominator removed and saved as a count', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await expect(page.getByRole('article', { name: 'Indicators' }).getByText(/\d+ indicators?/)).toBeVisible()
  await press(page, 'Open indicators', 'link')
  await expect(page.getByRole('heading', { name: 'Indicators', level: 1 })).toBeVisible()
  const grid = page.getByRole('grid', { name: 'Indicators' })
  await expect(grid.getByRole('row').filter({ hasText: 'ANC 1st visit before 12 weeks' }).getByText('In your draft')).toBeVisible()
  await grid.getByText('ANC 1st visit before 12 weeks').click()

  await expect(page.getByRole('heading', { name: 'ANC 1st visit before 12 weeks', level: 1 })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'No problems found while editing.' })).toBeVisible()
  const tryPanel = page.getByRole('complementary', { name: 'Try' })
  await tryPanel.getByLabel('Period', { exact: true }).fill('2026-03')
  await expect(tryPanel.getByText('75%')).toBeVisible()
  await expect(tryPanel.getByText('Numerator 3 of denominator 4')).toBeVisible()
  await expect(tryPanel.getByRole('heading', { name: 'Test patients: 3 of 11 counted' })).toBeVisible()
  await noViolations(page)

  // Removing the denominator makes it a count of the numerator's conditions.
  await press(page, 'Remove denominator')
  await expect(page.getByRole('combobox', { name: 'Calculation', exact: true })).toHaveValue('count')
  await expect(page.getByRole('group', { name: 'Denominator' })).toHaveCount(0)
  await expect(tryPanel.getByText(/^\d+ counted$/)).toBeVisible()
  // Save as test waits for the indicator to be saved, then keeps what Try gives in tests/.
  await expect(tryPanel.getByRole('button', { name: 'Save as test' })).toBeDisabled()
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByText('Saved.')).toBeVisible()
  await press(page, 'Save as test')
  await expect(tryPanel.getByText('Saved as tests/indicator-anc-first-visit-before-12-weeks-2026-03.json.', { exact: false })).toBeVisible()
  // Reopened, it is saved as a count.
  await press(page, 'Back to the Studio', 'link')
  await press(page, 'Open indicators', 'link')
  await grid.getByText('ANC 1st visit before 12 weeks').click()
  await expect(page.getByRole('combobox', { name: 'Calculation', exact: true })).toHaveValue('count')
})

test('an indicator that refers back to one referring to it is flagged as a circle', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open indicators', 'link')
  await page.getByRole('grid', { name: 'Indicators' }).getByText('ANC 1st visits', { exact: true }).click()
  await expect(page.getByRole('heading', { name: 'ANC 1st visits', level: 1 })).toBeVisible()
  await press(page, '+ in another indicator')
  await page.getByLabel('Indicator at Counted 1.2').selectOption({ label: 'ANC 1st visits with the next visit booked (https://health.go.ug/fhir/Measure/anc-first-visit-next-visit-booked|1.0.0)' })
  await expect(page.getByText(/The indicators refer to each other in a circle: this one → ANC 1st visits with the next visit booked → this one\./)).toBeVisible()
  await expect(page.getByLabel('Counted as FHIRPath')).toHaveValue(/'https:\/\/health\.go\.ug\/fhir\/Measure\/anc-first-visit-next-visit-booked\|1\.0\.0' in %indicators\.numerator/)
  await noViolations(page)
})
