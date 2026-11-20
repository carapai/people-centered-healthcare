import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #497: the OPD draft's author opens its triage Questionnaire in the form designer from the draft
// page, edits a question and sees the preview follow at once, adds a choice question, is told it
// has no options until it has one, places it in a section, reorders, and saves through the same
// If-Match path as the file editor. The JSON editor then shows what was saved. With axe.
test('the author designs the triage form, watching the preview, and saves it', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await expect(page.getByRole('heading', { name: 'Files' })).toBeVisible()
  // It opens from the changed Questionnaire.
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'Questionnaire-opd-triage.json in the form designer' }).click()
  await expect(page.getByRole('heading', { name: 'Form designer: OPD triage', level: 1 })).toBeVisible()
  const questions = page.getByRole('navigation', { name: 'Questions' })
  const inspector = page.getByRole('region', { name: 'Selected question' })
  const preview = page.getByRole('region', { name: /Live preview/ })
  await expect(preview.getByText('Triage category')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'No problems found while editing' })).toBeVisible()
  await noViolations(page)

  // Editing a question's text re-renders the preview at once.
  await questions.getByRole('button', { name: /^Triage category/ }).click()
  await inspector.getByLabel('Text', { exact: true }).fill('Triage category today')
  await expect(preview.getByText('Triage category today')).toBeVisible()

  // A new choice question with no options is a problem until it has one.
  await press(page, 'Add question')
  await inspector.getByLabel('Text', { exact: true }).fill('Arrived by ambulance')
  await inspector.getByRole('combobox', { name: 'Answer type' }).selectOption('coding')
  await expect(page.getByRole('button', { name: '“Arrived by ambulance” is a choice with no options' })).toBeVisible()
  await press(page, 'Add option')
  await inspector.getByLabel('Option 1', { exact: true }).fill('Yes, by ambulance')
  await expect(page.getByRole('heading', { name: 'No problems found while editing' })).toBeVisible()
  await expect(preview.getByText('Arrived by ambulance')).toBeVisible()

  // A section: the questions it does not place are listed, and show at the end of the form.
  await press(page, 'Add section')
  await page.getByLabel('Title of section 1').fill('Arrival')
  await inspector.getByRole('combobox', { name: 'Section', exact: true }).selectOption({ label: 'Arrival' })
  await expect(page.getByRole('button', { name: '“Vital signs” is in no section: it shows at the end of the form' })).toBeVisible()
  await expect(preview.getByText('Arrival', { exact: true })).toBeVisible()

  // Reordering among the questions in no section.
  await press(page, 'Move Triage category today up')
  const loose = questions.locator('div').filter({ has: page.getByRole('heading', { name: /In no section/ }) })
  await expect(loose.getByRole('listitem').first()).toContainText('Triage category today')
  await noViolations(page)

  await press(page, /^Save$/)
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()
  await expect(preview.getByText('Triage category today')).toBeVisible()
  await expect(page.getByText('Its layout is configuration/FormLayout-opd-triage.json.')).toBeVisible()

  // The JSON editor reads what was saved.
  await press(page, 'Open as JSON', 'link')
  const content = page.getByLabel('Content (JSON)')
  await expect(content).toHaveValue(/"text": "Triage category today"/)
  await expect(content).toHaveValue(/"display": "Yes, by ambulance"/)

  // Opened again, the form and its layout are as saved.
  await press(page, 'Back to the package', 'link')
  await expect(page.getByRole('link', { name: 'Open configuration/FormLayout-opd-triage.json' })).toBeVisible()
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Questionnaire-opd-triage.json in the form designer' }).click()
  await expect(questions.getByRole('heading', { name: 'Arrival' })).toBeVisible()
  await expect(questions.getByRole('button', { name: /^Arrived by ambulance/ })).toBeVisible()
})
