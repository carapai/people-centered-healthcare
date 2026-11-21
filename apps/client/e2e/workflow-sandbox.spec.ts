import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #499: the OPD draft's author plays its Workflow Definition in the Workflow Studio's sandbox.
// Every step comes from central, with the actions stubbed. They send an event the configuration
// accepts and see the active state lit on the canvas and in the outline, undo it, give a
// transition a guard, see the move that needs it not taken until they answer it, choose the
// payment gate's outcome, and save the run as a scenario in the package's tests/. Check now then
// runs it in the simulation, which passes: the answer to a guard the saved definition does not
// have is never asked. With axe.
/** Presses a sandbox button once the server's last answer is in (it is disabled while playing). */
async function play(page: Page, name: RegExp) {
  await expect(page.getByRole('button', { name })).toBeEnabled()
  await press(page, name)
}

test('the author plays the OPD draft in the sandbox, answers a guard, undoes a move and saves the run as a scenario', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Library-opd-visit.json in the Workflow Studio' }).click()
  await expect(page.getByRole('heading', { name: 'Workflow Studio: OPD visit', level: 1 })).toBeVisible()
  const states = page.getByRole('navigation', { name: 'States' })
  const canvas = page.getByRole('region', { name: 'Canvas' })

  // The start, from the server: reception is active and lit.
  await press(page, /^Sandbox$/)
  const sandbox = page.getByRole('region', { name: 'Sandbox' })
  await expect(sandbox.getByRole('heading', { name: 'At the start' })).toBeVisible()
  await expect(states.getByRole('button', { name: /^reception .*active/ })).toHaveAttribute('data-active', 'true')
  await expect(canvas.locator('.react-flow__node[data-id="reception"] [data-active=true]')).toBeVisible()
  await noViolations(page)

  // Send an event the configuration accepts; triage's task opens. Undo takes it back.
  await play(page, /^Send opd\.arrived$/)
  await expect(sandbox.getByRole('heading', { name: 'After move 1' })).toBeVisible()
  await expect(sandbox.getByText('triage (ready)')).toBeVisible()
  await expect(states.getByRole('button', { name: /^triage .*active/ })).toBeVisible()
  await expect(sandbox.getByText('Sent opd.arrived → triage')).toBeVisible()
  await play(page, /^Undo$/)
  await expect(sandbox.getByRole('heading', { name: 'At the start' })).toBeVisible()

  // A guard on triage's routine exit: the move that needs it is reported, not taken.
  await press(page, /^Inspect$/)
  await press(page, /^→ payment on opd\.triaged-routine/)
  const inspector = page.getByRole('region', { name: 'Inspect' })
  await inspector.getByRole('combobox', { name: 'Guard', exact: true }).selectOption('fhirpath')
  await inspector.getByRole('textbox', { name: 'FHIRPath expression', exact: true }).fill('%visit.paid-up')
  await press(page, /^Sandbox$/)
  await play(page, /^Send opd\.arrived$/)
  await expect(sandbox.getByRole('heading', { name: 'After move 1' })).toBeVisible()
  await play(page, /^Send opd\.triaged-routine$/)
  await expect(sandbox.getByRole('alert').filter({ hasText: 'The next move was not taken: answer %visit.paid-up first.' })).toBeVisible()
  await expect(sandbox.getByText('Sent opd.triaged-routine: not taken')).toBeVisible()
  await expect(sandbox.getByRole('button', { name: 'Send opd.triaged-urgent' })).toBeDisabled()

  // Answered, the move is taken; the gate passes and consultation is reached.
  await sandbox.getByRole('combobox', { name: 'Outcome of https://health.go.ug/fhir/FinancialGate/before-consultation' }).selectOption('pass')
  await sandbox.getByRole('combobox', { name: 'Answer to %visit.paid-up' }).selectOption('true')
  await expect(sandbox.getByRole('heading', { name: 'After move 2' })).toBeVisible()
  await expect(sandbox.getByText('Sent opd.triaged-routine → consultation')).toBeVisible()
  await expect(states.getByRole('button', { name: /^consultation .*active/ })).toBeVisible()
  await noViolations(page)

  // The run is saved as a scenario in tests/. The guard is left unsaved: the package's other
  // scenarios do not answer it.
  await sandbox.getByLabel('Scenario name').fill('Routine visit after the guard')
  await press(page, 'Save as scenario')
  await expect(sandbox.getByRole('status').filter({ hasText: /^Saved as tests\/routine-visit-after-the-guard(-\d+)?\.json\. Submission runs it\.$/ })).toBeVisible()

  // Check now runs it, with the package's other scenarios, in the simulation: they pass.
  await press(page, 'Back to the package', 'link')
  await expect(page.getByRole('heading', { name: 'Who must act' })).toBeVisible()
  await press(page, 'Check now')
  await expect(page.getByRole('status').filter({ hasText: 'Done.' })).toBeVisible()
  await expect(page.getByText(/^Simulation passed/)).toBeVisible()
})
