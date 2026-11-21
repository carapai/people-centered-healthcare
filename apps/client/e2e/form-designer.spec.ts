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
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.opd', { exact: false }).click()
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

// #497 part 2: what an answer becomes, edited in the package's Extraction Mapping, and a form
// imported from FHIR JSON, R4 through the conversion report. With axe.
test('the author sets what answers become a record, and imports an R4 form through the conversion report', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'Questionnaire-opd-triage.json in the form designer' }).click()
  await expect(page.getByText('What answers become is in Library-opd-triage-mapping.json.')).toBeVisible()
  const questions = page.getByRole('navigation', { name: 'Questions' })
  const inspector = page.getByRole('region', { name: 'Selected question' })
  const preview = page.getByRole('region', { name: /Live preview/ })

  // A temperature already becomes an Observation coded by LOINC; its display is edited.
  await questions.getByRole('button', { name: /^Temperature/ }).click()
  await expect(inspector.getByText('Record 1: Observation')).toBeVisible()
  await expect(inspector.getByLabel('Code of record 1')).toHaveValue('8310-5')
  await inspector.getByLabel('Display of record 1').fill('Body temperature (oral)')

  // The triage category becomes no record until one is added, and needs a code.
  await questions.getByRole('button', { name: /^Triage category/ }).click()
  await expect(inspector.getByText('No record: the answer stays in the response.')).toBeVisible()
  await press(page, 'Record it as an Observation')
  await expect(page.getByRole('button', { name: /^What “Triage category.*” becomes has no code$/ })).toBeVisible()
  await inspector.getByLabel('Code of record 1').fill('11283-9')
  await inspector.getByLabel('Display of record 1').fill('Acuity assessment')
  await expect(page.getByRole('button', { name: /becomes has no code$/ })).toHaveCount(0)
  await noViolations(page)
  await press(page, /^Save$/)
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()

  // The mapping holds both.
  await press(page, 'Back to the package', 'link')
  await press(page, 'Open Library-opd-triage-mapping.json', 'link')
  const content = page.getByLabel('Content (JSON)')
  await expect(content).toHaveValue(/"display": "Body temperature \(oral\)"/)
  await expect(content).toHaveValue(/"code": "11283-9"/)

  // Import: an R4 form read as R5 is refused; read as R4 it is converted, with its report.
  await press(page, 'Back to the package', 'link')
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Questionnaire-opd-triage.json in the form designer' }).click()
  await press(page, 'Import FHIR JSON')
  const panel = page.getByRole('region', { name: 'Import FHIR JSON' })
  const r4 = {
    resourceType: 'Questionnaire',
    url: 'http://smart.who.int/anc/Questionnaire/ANCDT01',
    version: '1.0.0',
    status: 'active',
    title: 'ANC danger signs',
    item: [{ linkId: 'danger', type: 'choice', text: 'Danger signs', answerOption: [{ valueCoding: { code: 'bleeding', display: 'Vaginal bleeding' } }] }],
  }
  await panel.getByLabel('Questionnaire (FHIR JSON)').fill(JSON.stringify(r4, null, 2))
  await press(page, 'Read it')
  await expect(panel.getByRole('alert')).toHaveText('It uses R4’s choice types: read it as R4.')
  await panel.getByLabel('R4, converted to R5 with a report').check()
  await press(page, 'Read it')
  const report = panel.getByRole('list', { name: /^Conversion report/ })
  await expect(report).toContainText('“danger”: R4 choice became coding, options only')
  await expect(report).toContainText('Its source http://smart.who.int/anc/Questionnaire/ANCDT01|1.0.0 is kept in derivedFrom')
  await noViolations(page)
  await press(page, 'Replace the form')
  await expect(page.getByRole('status').filter({ hasText: 'The form was replaced by the import. Save to keep it.' })).toBeVisible()
  await expect(preview.getByText('Danger signs')).toBeVisible()
  await expect(preview.getByText('Vaginal bleeding')).toBeVisible()
  await expect(questions.getByRole('button', { name: /^Danger signs/ })).toBeVisible()
  // What the mapping recorded of the old form is listed until it is dealt with.
  await expect(page.getByText('The Extraction Mapping records “temperature”, which is no longer in the form')).toBeVisible()
})

// #497 part 3: a choice question bound to a ValueSet through the picker. The author starts a
// ValueSet of the triage categories in the terminology browser, then binds it from the form
// designer: the picker finds the draft, previews its codes, suggests radio buttons, and the
// binding records `url|version`. The preview offers the ValueSet's codes. With axe.
test('the author binds the triage category to a ValueSet through the picker', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'CodeSystem-triage-category.json in the terminology browser' }).click()
  const valueSet = page.getByRole('region', { name: 'ValueSet', exact: true })
  const concepts = page.getByRole('region', { name: 'Concepts of TriageCategory' })
  await valueSet.getByLabel('Id', { exact: true }).fill('triage-every')
  await valueSet.getByLabel('Title', { exact: true }).fill('Every triage category')
  await press(page, 'Start the ValueSet')
  for (const code of ['emergency', 'urgent', 'routine']) {
    await concepts.getByRole('button', { name: new RegExp(`^${code}`) }).click()
    await press(page, new RegExp(`^Add ${code}$`))
  }
  await expect(valueSet.getByRole('heading', { name: 'Expansion: 3 codes' })).toBeVisible()
  await press(page, 'Save the ValueSet')
  await page.waitForURL(/\/terminology\/ValueSet-triage-every\.json$/)

  await press(page, 'Back to the package', 'link')
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'Questionnaire-opd-triage.json in the form designer' }).click()
  const questions = page.getByRole('navigation', { name: 'Questions' })
  const inspector = page.getByRole('region', { name: 'Selected question' })
  const preview = page.getByRole('region', { name: /Live preview/ })
  await questions.getByRole('button', { name: /^Triage category/ }).click()

  // The picker finds the draft ValueSet, previews its codes and suggests a widget.
  await press(page, 'Bind a ValueSet')
  const picker = inspector.getByRole('group', { name: 'ValueSet picker' })
  await picker.getByLabel('Search ValueSets').fill('every')
  await picker.getByRole('button', { name: /^Every triage category 1\.0\.0 · Draft in this package/ }).click()
  await expect(picker.getByText('https://health.go.ug/fhir/ValueSet/triage-every|1.0.0')).toBeVisible()
  await expect(picker.getByText('Expansion: 3 codes')).toBeVisible()
  await expect(picker.getByRole('list', { name: /^Codes of/ })).toContainText('Urgent: seen next')
  await expect(picker.getByText('Suggested widget: radio buttons')).toBeVisible()
  await expect(picker.getByText('Binding replaces the question’s own options with the ValueSet’s codes.')).toBeVisible()
  await noViolations(page)
  await press(page, 'Bind Every triage category')

  // Bound, its own options are gone and the preview offers the ValueSet's codes.
  await expect(inspector.getByText('Bound to the ValueSet https://health.go.ug/fhir/ValueSet/triage-every|1.0.0')).toBeVisible()
  await expect(inspector.getByLabel('Option 1', { exact: true })).toHaveCount(0)
  await expect(preview.getByRole('radio', { name: 'Routine' })).toBeVisible()
  await expect(page.getByRole('button', { name: /^“Triage category.*” is a choice with no options$/ })).toHaveCount(0)
  await noViolations(page)
  await press(page, /^Save$/)
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()

  await press(page, 'Open as JSON', 'link')
  await expect(page.getByLabel('Content (JSON)')).toHaveValue(/"answerValueSet": "https:\/\/health\.go\.ug\/fhir\/ValueSet\/triage-every\|1\.0\.0"/)
})
