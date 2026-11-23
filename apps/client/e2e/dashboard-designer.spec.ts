import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #879: the Dashboard designer. The HMIS Reporting package (ug.moh.hmis) is the author's draft,
// with its standard Dashboard *ANC overview*, over the ANC indicators and test patients of the
// ANC package, which is a draft too: the preview computes with them, and Validation, as
// publication would, asks for them to be published.

test('ANC overview opens with one Visualization open at a time, previewed over test patients, with publication’s problems', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open dashboards', 'link')
  // The local backend also publishes a sample Reporting package with an *ANC overview*: open the draft's.
  await page.getByRole('grid', { name: 'Dashboards' }).getByRole('row').filter({ hasText: 'In your draft' }).getByText('ANC overview').click()
  await expect(page.getByRole('heading', { name: 'ANC overview', level: 1 })).toBeVisible()

  const outline = page.getByRole('navigation', { name: 'Outline' })
  const byArea = outline.getByRole('button', { name: /^ANC 1st visits by area/ })
  const byAge = outline.getByRole('button', { name: /^ANC 1st visits by age band/ })
  await expect(byArea).toHaveAttribute('aria-expanded', 'true')
  await expect(byAge).toHaveAttribute('aria-expanded', 'false')
  await byAge.click()
  await expect(byAge).toHaveAttribute('aria-expanded', 'true')
  await expect(byArea).toHaveAttribute('aria-expanded', 'false')

  // The preview draws each Visualization with the explorer's charts, over the ANC test patients
  // as of a day near their visits.
  const preview = page.getByRole('region', { name: 'Preview' })
  await preview.getByLabel('As of').fill('2026-04-30')
  await expect(preview.getByText(/over 11 test patients/)).toBeVisible()
  await expect(preview.getByRole('article', { name: 'ANC 1st visits by area' }).getByText('2025-11 to 2026-04')).toBeVisible()
  await expect(preview.getByRole('article', { name: 'ANC 1st visits this quarter' }).getByRole('list', { name: 'Single numbers' })).toBeVisible()

  // Validation is publication's: the ANC indicators are drafts, and a title cleared is flagged
  // in its Visualization.
  const validation = page.getByRole('region', { name: 'Validation' })
  await expect(validation.getByText('Visualization 1 (ANC 1st visits by area): no published indicator is https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0')).toBeVisible()
  await outline.getByLabel('Visualization title').fill('')
  await expect(validation.getByText('Visualization 2 has no title')).toBeVisible()
  await noViolations(page)
})

test('New dashboard creates a standard Dashboard in the Reporting package and the designer saves it', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open dashboards', 'link')
  await press(page, 'New dashboard')
  await page.getByLabel('Title').fill('ANC monthly check')
  await press(page, 'Create and open')
  await expect(page.getByRole('heading', { name: 'ANC monthly check', level: 1 })).toBeVisible()
  const validation = page.getByRole('region', { name: 'Validation' })
  await expect(validation.getByText('the Dashboard has no Visualizations')).toBeVisible()
  await press(page, '+ visualization')
  await expect(validation.getByText('the Dashboard has no Visualizations')).toHaveCount(0)
  await expect(page.getByRole('navigation', { name: 'Outline' }).getByRole('button', { name: /^New chart/ })).toHaveAttribute('aria-expanded', 'true')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByText('Saved.')).toBeVisible()
  await noViolations(page)
})
