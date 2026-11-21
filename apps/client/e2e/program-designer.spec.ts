import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #500: the author of the synthetic ART program draft (e2e/fixtures/art) opens its plan in the
// Program Designer from the draft page. The journey board has one column per state of the care
// workflow, the strip what applies to the whole program, and the package panel the files by
// kind with what is still missing. Reordering the regimen templates saves the plan through the
// same If-Match path as the file editor, and it is kept. Create on the missing exit form adds it
// to the draft and opens the form designer; back on the board it is in this draft. With axe.
const B = 'https://health.go.ug/fhir'

async function openDesigner(page: Page) {
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'PlanDefinition-art-program.json in the Program Designer' }).click()
  await expect(page.getByRole('heading', { name: 'Program Designer: HIV care and antiretroviral therapy (ART)', level: 1 })).toBeVisible()
  await expect(page.getByRole('region', { name: 'Journey' }).getByRole('heading', { level: 3 })).toHaveCount(6)
}

test('the ART program assembles on the journey board; its regimen rules are reordered and a missing form is created', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.art', { exact: false }).click()
  await openDesigner(page)

  // One column per state of the care workflow, in its order.
  const journey = page.getByRole('region', { name: 'Journey' })
  await expect(journey.getByRole('heading', { level: 3 })).toHaveText(['1. enrolment', '2. baseline', '3. initiation', '4. follow-up', '5. outcome', '6. closed · final'])
  const strip = page.getByRole('region', { name: 'The whole program' })
  await expect(strip.getByText("%patient.extension('hiv-status').value = 'positive'")).toBeVisible()
  await expect(strip.getByText(`${B}/Library/art-care|1.0.0`)).toBeVisible()
  await expect(strip.getByText('No override points: a facility admin may change nothing')).toBeVisible()
  const initiation = journey.getByRole('listitem').filter({ has: page.getByRole('heading', { name: '3. initiation' }) })
  await expect(initiation.getByText('Regimen templates (3)')).toBeVisible()
  await expect(initiation.getByText('Cites Consolidated HIV guidelines, 2024 (synthetic test source)')).toHaveCount(3)
  const followUp = journey.getByRole('listitem').filter({ has: page.getByRole('heading', { name: '4. follow-up' }) })
  await expect(followUp.getByText('art.missed-visit after P28D')).toBeVisible()
  await expect(followUp.getByText('After P28D → outcome')).toBeVisible()
  const panel = page.getByRole('complementary', { name: /^Package/ })
  await expect(panel.getByRole('heading', { name: 'Still missing (3)' })).toBeVisible()
  await expect(panel.getByText(`Form ${B}/Questionnaire/art-exit|1.0.0 does not exist yet`)).toBeVisible()
  await expect(panel.getByText(`${B}/Measure/art-tracing|1.0.0 (missing)`)).toBeVisible()
  await noViolations(page)

  // The TB rule moves above the paediatric one, with the keyboard; the plan is saved.
  await press(page, 'Move “TLD with an extra DTG 50 mg twelve hours later” up')
  await expect(page.getByRole('status').filter({ hasText: 'The order is saved.' })).toBeVisible()
  await expect(initiation.getByText('2. TLD with an extra DTG 50 mg twelve hours later')).toBeVisible()

  // Opened again, the order is as saved.
  await press(page, 'Back to the package', 'link')
  await openDesigner(page)
  await expect(page.getByText('2. TLD with an extra DTG 50 mg twelve hours later')).toBeVisible()
  await expect(page.getByText('3. ABC/3TC with DTG, by weight band')).toBeVisible()

  // Create on the missing exit form adds it to the draft and opens the form designer.
  await press(page, `Create ${B}/Questionnaire/art-exit|1.0.0`)
  await expect(page.getByRole('heading', { name: 'Form designer: art-exit', level: 1 })).toBeVisible()
  await press(page, 'Back to the package', 'link')
  await expect(page.getByRole('region', { name: 'Files' }).getByText('Questionnaire-art-exit.json')).toBeVisible()
  await openDesigner(page)
  const outcome = page.getByRole('region', { name: 'Journey' }).getByRole('listitem').filter({ has: page.getByRole('heading', { name: '5. outcome' }) })
  await expect(outcome.getByText('in this draft, 1.0.0')).toBeVisible()
  await expect(outcome.getByRole('link', { name: `Open ${B}/Questionnaire/art-exit|1.0.0` })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Still missing (2)' })).toBeVisible()
  await noViolations(page)
})
