import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #846 (ADR 0093): a transition's guard may name a decision table and the outcome it requires.
// The OPD draft's author opens the OPD triage route (packages/reference-packages/opd), whose exits
// from triage are guarded by the triage-priority table's outcome. The inspector shows the table and
// the required outcome, picked from the table's outcomes, and opens the table in the
// decision-table editor. In the sandbox, the move that needs the table waits for its outcome;
// given one, the guarded transition fires. With axe.

/** Presses a sandbox button once the server's last answer is in (it is disabled while playing). */
async function play(page: Page, name: RegExp) {
  await expect(page.getByRole('button', { name })).toBeEnabled()
  await press(page, name)
}

test('the author picks a decision table and its outcome as a guard and plays it in the sandbox', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Library-opd-triage-route.json in the Workflow Studio' }).click()
  await expect(page.getByRole('heading', { name: 'Workflow Studio: OPD triage route', level: 1 })).toBeVisible()
  const states = page.getByRole('navigation', { name: 'States' })

  // The inspector shows the guard as a table and the outcome it requires, from the table's own.
  await states.getByRole('button', { name: /^→ emergency on opd\.triaged \[triage-priority = emergency\]/ }).focus()
  await page.keyboard.press('Enter')
  const inspector = page.getByRole('region', { name: 'Inspect' })
  await expect(inspector.getByRole('combobox', { name: 'Guard', exact: true })).toHaveValue('table')
  await expect(inspector.getByRole('combobox', { name: 'Decision table', exact: true })).toHaveValue('triage-priority')
  const outcome = inspector.getByRole('combobox', { name: 'Required outcome', exact: true })
  await expect(outcome).toHaveValue('emergency')
  await expect(outcome.getByRole('option')).toHaveText(['emergency', 'urgent', 'routine'])
  await noViolations(page)

  // It opens the table in the decision-table editor.
  await press(page, 'Open triage-priority in the decision-table editor')
  await expect(page.getByRole('heading', { name: 'Decision table triage-priority' })).toBeVisible()
  await press(page, 'Back to the statechart')

  // In the sandbox, the move that needs the table waits for its outcome.
  await press(page, /^Sandbox$/)
  const sandbox = page.getByRole('region', { name: 'Sandbox' })
  await expect(sandbox.getByRole('heading', { name: 'At the start' })).toBeVisible()
  await play(page, /^Send opd\.triaged$/)
  await expect(sandbox.getByRole('alert').filter({ hasText: 'The next move was not taken: give the outcome of triage-priority first.' })).toBeVisible()

  // Given `emergency`, the guarded transition fires.
  await sandbox.getByRole('combobox', { name: 'Outcome of triage-priority' }).selectOption('emergency')
  await expect(sandbox.getByRole('heading', { name: 'After move 1' })).toBeVisible()
  await expect(sandbox.getByText('Sent opd.triaged → emergency')).toBeVisible()
  await expect(states.getByRole('button', { name: /^emergency .*active/ })).toBeVisible()
  // `routine` sends the patient to the cashier instead.
  await sandbox.getByRole('combobox', { name: 'Outcome of triage-priority' }).selectOption('routine')
  await expect(sandbox.getByText('Sent opd.triaged → payment')).toBeVisible()
  await noViolations(page)
})
