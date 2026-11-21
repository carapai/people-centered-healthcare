import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #551: MoH's Age Band Sets are published in the national core package (ug.moh.core). The Studio
// lists them in their own section and edits one as a small table, flagging gaps and overlaps as
// the author types; a band change takes the next major version of the set.

test('an Age Band Set is edited as a table, its overlaps flagged and a band change a major version', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await expect(page.getByRole('article', { name: 'Age band sets' }).getByText(/\d+ age band sets?/)).toBeVisible()
  await press(page, 'Open age band sets', 'link')
  await expect(page.getByRole('heading', { name: 'Age band sets', level: 1 })).toBeVisible()
  const grid = page.getByRole('grid', { name: 'Age band sets' })
  await expect(grid.getByRole('row').filter({ hasText: 'MoH outpatient age bands' }).getByText('Published')).toBeVisible()
  await grid.getByText('MoH ANC age bands').click()

  // Published: read only, contiguous.
  await expect(page.getByRole('heading', { name: 'MoH ANC age bands', level: 1 })).toBeVisible()
  const bands = page.getByRole('table', { name: 'Bands' })
  await expect(bands.getByRole('cell', { name: '25+ years' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'No gaps or overlaps.' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Save' })).toHaveCount(0)
  await noViolations(page)

  // Edit starts the next version of the package; widening the first band overlaps the second.
  await press(page, 'Edit')
  await page.waitForURL(/ug\.moh\.core%401\.1\.0-draft/)
  await page.getByLabel('Band 1: To').fill('15')
  await expect(page.getByText('Overlap between 10–15 years and 15–19 years: 15 years are in both.')).toBeVisible()
  await expect(bands.locator('tr[data-flagged]')).toHaveCount(2)
  await expect(page.getByText(/Version 2\.0\.0: the bands changed since 1\.0\.0, so it is a major version/)).toBeVisible()
  await noViolations(page)

  await page.getByLabel('Band 2: From').fill('16')
  await expect(page.getByRole('heading', { name: 'No gaps or overlaps.' })).toBeVisible()
  await press(page, 'Save')
  await expect(page.getByText('Saved.')).toBeVisible()
})
