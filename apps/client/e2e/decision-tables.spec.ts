import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #516: the OPD draft's author creates a decision table in the Workflow Studio. They declare the
// data the rules read, build the rule any(all(age < 5, danger signs), all(pregnant, urgent)) as
// a tree, saved as one FHIRPath expression, and two routine rules, the second shadowed by the
// first and marked as never firing until it is moved above it. They try a patient on the
// server, save the case as a test, see it fail when an edit changes its outcome, and save the
// definition. With axe.

const SICK_CHILD_OR_URGENT_PREGNANCY = "(%patient.age < 5 and %triage.danger = true) or (%patient.pregnant = true and %triage.category = 'urgent')"

/** Sets condition `at` of the open rule. */
async function condition(page: Page, at: string, input: string, kind: 'yes or no' | 'number' | 'code', op: string, value: string) {
  await page.getByLabel(`Input of condition ${at}`, { exact: true }).selectOption(input)
  await page.getByLabel(`Type of condition ${at}`, { exact: true }).selectOption({ label: kind })
  await page.getByLabel(`Operator of condition ${at}`, { exact: true }).selectOption(op)
  const field = page.getByLabel(`Value of condition ${at}`, { exact: true })
  if (kind === 'yes or no') await field.selectOption(value)
  else await field.fill(value)
}

async function requirement(page: Page, name: string, resourceType: string, paths: string[]) {
  await page.getByLabel('Name', { exact: true }).fill(name)
  await page.getByLabel('Resource type').fill(resourceType)
  await press(page, 'Add a data requirement')
  for (const path of paths) {
    await page.getByLabel(`Path to add to ${name}`).fill(path)
    await press(page, `Add the path to ${name}`)
  }
}

test('the author builds named rules as condition trees, tries a patient, saves a test case and sees the shadowed rule never fire', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Library-opd-visit.json in the Workflow Studio' }).click()
  await expect(page.getByRole('heading', { name: 'Workflow Studio: OPD visit', level: 1 })).toBeVisible()

  await press(page, 'New decision table')
  await expect(page.getByRole('heading', { name: 'Decision table table' })).toBeVisible()
  await page.getByLabel('Table id').fill('route')
  await expect(page.getByRole('heading', { name: 'Decision table route' })).toBeVisible()
  await requirement(page, 'patient', 'Patient', ['Patient.age', 'Patient.pregnant'])
  await requirement(page, 'triage', 'Observation', ['Observation.category', 'Observation.danger'])

  // Rule 1: any(all(age < 5, danger signs), all(pregnant, urgent)), one FHIRPath expression.
  await press(page, 'Add rule')
  await expect(page.getByRole('heading', { name: 'Rule 1' })).toBeVisible()
  await page.getByLabel('Rule name').fill('Sick child or urgent pregnancy')
  await page.getByLabel('Outcome', { exact: true }).fill('bypass')
  await page.getByLabel('Join of group 1', { exact: true }).selectOption('any')
  await press(page, /^Add a group to group 1$/)
  await press(page, /^Add a group to group 1$/)
  await press(page, /^Add a condition to group 1\.1$/)
  await press(page, /^Add a condition to group 1\.1$/)
  await condition(page, '1.1.1', '%patient.age', 'number', '<', '5')
  await condition(page, '1.1.2', '%triage.danger', 'yes or no', '=', 'true')
  await press(page, /^Add a condition to group 1\.2$/)
  await press(page, /^Add a condition to group 1\.2$/)
  await condition(page, '1.2.1', '%patient.pregnant', 'yes or no', '=', 'true')
  await condition(page, '1.2.2', '%triage.category', 'code', '=', 'urgent')
  await expect(page.getByText(SICK_CHILD_OR_URGENT_PREGNANCY, { exact: true })).toBeVisible()

  // Routine patients pay first; the routine infant below it never fires.
  await press(page, 'Add rule')
  await page.getByLabel('Rule name').fill('Routine')
  await page.getByLabel('Outcome', { exact: true }).fill('normal')
  await press(page, /^Add a condition to group 1$/)
  await condition(page, '1.1', '%triage.category', 'code', '=', 'routine')
  await press(page, 'Add rule')
  await page.getByLabel('Rule name').fill('Routine infant')
  await page.getByLabel('Outcome', { exact: true }).fill('bypass')
  await press(page, /^Add a condition to group 1$/)
  await press(page, /^Add a condition to group 1$/)
  await condition(page, '1.1', '%triage.category', 'code', '=', 'routine')
  await condition(page, '1.2', '%patient.age', 'number', '<', '1')
  const rules = page.getByRole('navigation', { name: 'Rules' })
  await expect(page.getByText('Never fires, a rule above always matches first: “Routine infant”')).toBeVisible()
  await expect(rules.getByRole('button', { name: /^3\. Routine infant .*never fires/ })).toBeVisible()
  // With no default, what no rule covers is a gap; a default covers it.
  await expect(page.getByText(/combinations? match(es)? no rule, e\.g\./)).toBeVisible()
  await page.getByLabel('When no rule holds').fill('normal')
  await expect(page.getByText('Every combination is covered')).toBeVisible()
  // Moved above the rule that shadows it, it fires.
  await press(page, 'Move “Routine infant” up')
  await expect(page.getByText('Every rule can fire')).toBeVisible()
  await expect(rules.getByRole('button', { name: /^2\. Routine infant/ })).toBeVisible()

  // Try a patient: a child of 3 with danger signs bypasses payment, from rule 1.
  const tryPanel = page.getByRole('complementary', { name: 'Try a patient' })
  await tryPanel.getByLabel('%patient.age').fill('3')
  await tryPanel.getByLabel('%triage.danger').selectOption('true')
  await tryPanel.getByLabel('%patient.pregnant').selectOption('false')
  await tryPanel.getByLabel('%triage.category').fill('routine')
  await expect(tryPanel.getByRole('status').filter({ hasText: 'Outcome: bypass, from' })).toBeVisible()
  await expect(rules.getByRole('button', { name: /^1\. Sick child or urgent pregnancy .*fires for this patient/ })).toBeVisible()
  await press(page, /^“Sick child or urgent pregnancy”$/)
  await expect(page.getByRole('heading', { name: 'Rule 1' })).toBeVisible()
  await expect(page.getByRole('group', { name: 'Group 1.1: all of these' }).getByRole('img', { name: 'holds for this patient' }).first()).toBeVisible()
  await expect(page.getByRole('group', { name: 'Group 1.2: all of these' }).getByRole('img', { name: 'does not hold for this patient' }).first()).toBeVisible()
  await noViolations(page)

  // Save as test keeps the inputs and today's outcome in the package's tests/.
  await tryPanel.getByLabel('Name this case').fill('Child of 3 with danger signs')
  await press(page, 'Save as test')
  await expect(page.getByRole('status').filter({ hasText: 'Saved as tests/route-child-of-3-with-danger-signs.json.' })).toBeVisible()
  await expect(tryPanel.getByRole('button', { name: 'Passes: Child of 3 with danger signs: expects bypass, gets bypass' })).toBeVisible()
  // An edit that changes its outcome fails it.
  await page.getByLabel('Value of condition 1.1.1', { exact: true }).fill('1')
  await expect(tryPanel.getByRole('button', { name: 'Fails: Child of 3 with danger signs: expects bypass, gets normal' })).toBeVisible()
  await expect(page.getByText('1 of 1 test case fails')).toBeVisible()
  await page.getByLabel('Value of condition 1.1.1', { exact: true }).fill('5')
  await expect(page.getByText('0 of 1 test case fails')).toBeVisible()

  await press(page, /^Save$/)
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()

  // Opened again, the table reads back as it was built.
  await press(page, 'Back to the package', 'link')
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Library-opd-visit.json in the Workflow Studio' }).click()
  await expect(page.getByRole('heading', { name: 'No publication problems found while editing' })).toBeVisible()
  await press(page, 'route: 3 rules')
  await expect(page.getByRole('heading', { name: 'Rule 1' })).toBeVisible()
  await expect(page.getByLabel('Rule name')).toHaveValue('Sick child or urgent pregnancy')
  await expect(page.getByText(SICK_CHILD_OR_URGENT_PREGNANCY, { exact: true })).toBeVisible()
  await expect(page.getByLabel('Join of group 1', { exact: true })).toHaveValue('any')
  await noViolations(page)
})
