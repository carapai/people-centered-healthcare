import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #559: the Report designer. The HMIS Reporting package (ug.moh.hmis) is the author's draft, over
// the ANC indicators and test patients of the ANC package, which is a draft too: the preview
// computes with them, and Validation asks for them to be published.

const MEDIAN = 'https://health.go.ug/fhir/Measure/anc-gestational-age-at-first-visit|1.0.0'

test('one section is open at a time, a preview cell opens its section, and problems appear as they are introduced', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open reports', 'link')
  await page.getByRole('grid', { name: 'Reports' }).getByText('HMIS 105 · 2 Antenatal care').click()
  await expect(page.getByRole('heading', { name: 'HMIS 105 · 2 Antenatal care', level: 1 })).toBeVisible()

  const outline = page.getByRole('navigation', { name: 'Outline' })
  const attendance = outline.getByRole('button', { name: /^2\.1 ANC attendance/ })
  const timing = outline.getByRole('button', { name: /^2\.2 Timing of the 1st visit/ })
  await expect(attendance).toHaveAttribute('aria-expanded', 'true')
  await expect(timing).toHaveAttribute('aria-expanded', 'false')
  await timing.click()
  await expect(timing).toHaveAttribute('aria-expanded', 'true')
  await expect(attendance).toHaveAttribute('aria-expanded', 'false')

  // The preview computes the Report over the ANC test patients; a cell opens its section.
  const preview = page.getByRole('region', { name: 'Preview' })
  await expect(preview.getByText(/over 11 test patients/)).toBeVisible()
  const firstVisits = preview.getByRole('table', { name: '2.1 ANC attendance' }).getByRole('row', { name: /^ANC 1st visit for women/ })
  await firstVisits.getByRole('button').last().click()
  await expect(attendance).toHaveAttribute('aria-expanded', 'true')
  await expect(timing).toHaveAttribute('aria-expanded', 'false')
  await expect(page.getByRole('region', { name: 'Selected cell' }).getByText('Counted: 4')).toBeVisible()

  // Validation, beside the outline: the ANC indicators are drafts, so each row asks for its
  // indicator to be published; a row added is checked as it is added, and the median it adds to
  // a Report that runs at district level is flagged facility only.
  const validation = page.getByRole('region', { name: 'Validation' })
  await expect(validation.getByText(/no published indicator is https:\/\/health\.go\.ug\/fhir\/Measure\/anc-first-visits\|1\.0\.0/)).toBeVisible()
  await expect(validation.getByText(/row 3/)).toHaveCount(0)
  await outline.getByLabel('Indicator to add').selectOption(MEDIAN)
  await outline.getByRole('button', { name: '+ indicator' }).click()
  await expect(validation.getByText(/section 1 \(2\.1 ANC attendance\)'s row 3 \(.+\): no published indicator is https:\/\/health\.go\.ug\/fhir\/Measure\/anc-gestational-age-at-first-visit\|1\.0\.0/)).toBeVisible()
  await expect(attendance).toContainText('3 problems')
  await expect(outline.getByText('facility only: it cannot be rebuilt from facility summaries')).toBeVisible()
  await noViolations(page)
})

test('New report creates a Report in the Reporting package and the designer saves it', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open reports', 'link')
  await press(page, 'New report')
  await page.getByLabel('Title').fill('ANC quarterly check')
  await press(page, 'Create and open')
  await expect(page.getByRole('heading', { name: 'ANC quarterly check', level: 1 })).toBeVisible()
  const validation = page.getByRole('region', { name: 'Validation' })
  await expect(validation.getByText('the Report has no sections')).toBeVisible()
  await press(page, '+ table section')
  await expect(validation.getByText(/section 1 \(New table section\) has no indicator rows/)).toBeVisible()
  await page.getByRole('navigation', { name: 'Outline' }).getByRole('button', { name: '+ indicator' }).click()
  await expect(validation.getByText(/has no indicator rows/)).toHaveCount(0)
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByText('Saved.')).toBeVisible()
  await noViolations(page)
})
