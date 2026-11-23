import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #537: the indicator editor. The ANC reference package (ug.moh.anc) is the author's draft, with
// its indicators and test patients. Try evaluates the unsaved definition on central over them.

// #864: Try marks each condition of the numerator and denominator held or not on every test
// patient's records, and the editor shows, read-only, where Reports lay the indicator out (the
// local backend drafts the HMIS Reporting package, ug.moh.hmis).
test("Try marks each condition held or not, and the indicator's HMIS mapping is shown", async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open indicators', 'link')
  await page.getByRole('grid', { name: 'Indicators' }).getByText('ANC 1st visit before 12 weeks', { exact: true }).click()
  await expect(page.getByRole('heading', { name: 'ANC 1st visit before 12 weeks', level: 1 })).toBeVisible()
  const tryPanel = page.getByRole('complementary', { name: 'Try' })
  await tryPanel.getByLabel('Period', { exact: true }).fill('2026-03')
  await expect(tryPanel.getByText('Numerator 3 of denominator 4')).toBeVisible()
  const patients = tryPanel.getByRole('region', { name: /Test patients/ })

  // A counted patient: the gestational age under 12 held, and so did the denominator's conditions.
  const counted = patients.getByRole('listitem').filter({ has: page.locator('summary', { hasText: /: counted 1/ }) }).first()
  await counted.locator('summary').click()
  // Its counted visit.
  const visit = counted.getByRole('listitem').filter({ hasText: /— counted/ }).first()
  await expect(visit.getByText('Numerator')).toBeVisible()
  await expect(visit.getByText('Denominator')).toBeVisible()
  const ga = visit.getByRole('listitem').filter({ hasText: '< 12' })
  await expect(ga).toContainText('held:')
  await expect(ga).not.toContainText('not held')
  await expect(visit.getByRole('listitem').filter({ hasText: "%patient.gender = 'female'" })).not.toContainText('not held')

  // A patient in the denominator only: the gestational age condition did not hold.
  const notCounted = patients.getByRole('listitem').filter({ has: page.locator('summary', { hasText: /not counted · denominator 1/ }) }).first()
  await notCounted.locator('summary').click()
  const inDenominator = notCounted.getByRole('listitem').filter({ hasText: /— in the denominator only/ }).first()
  await expect(inDenominator.getByRole('listitem').filter({ hasText: '< 12' })).toContainText('not held:')
  await noViolations(page)

  // The HMIS mapping: the HMIS 105 row laying it out, in the HMIS package's draft.
  const mapping = page.getByRole('region', { name: 'HMIS mapping' })
  const row = mapping.getByRole('link', { name: 'HMIS 105 · 2 Antenatal care, 2.2 Timing of the 1st visit: ANC 1st visit before 12 weeks (%)' })
  await expect(row).toBeVisible()
  await expect(mapping.getByText(/ug\.moh\.hmis@[\d.]+(-draft)? \(draft\) · version 1\.0\.0/)).toBeVisible()
  await row.click()
  await expect(page).toHaveURL(/\/studio\/ug\.moh\.hmis(@|%40)[^/]+\/reports\/configuration\/reports\/hmis-105-anc\.json/)
})

// Runs after the test above: it saves the indicator as a count.
test('an indicator is tried over the test patients, its denominator removed and saved as a count', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await expect(page.getByRole('article', { name: 'Indicators' }).getByText(/\d+ indicators?/)).toBeVisible()
  await press(page, 'Open indicators', 'link')
  await expect(page.getByRole('heading', { name: 'Indicators', level: 1 })).toBeVisible()
  const grid = page.getByRole('grid', { name: 'Indicators' })
  await expect(grid.getByRole('row').filter({ hasText: 'ANC 1st visit before 12 weeks' }).getByText('In your draft')).toBeVisible()
  await grid.getByText('ANC 1st visit before 12 weeks', { exact: true }).click()

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
  await grid.getByText('ANC 1st visit before 12 weeks', { exact: true }).click()
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
