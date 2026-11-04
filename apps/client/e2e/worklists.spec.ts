import { randomUUID } from 'node:crypto'

import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #99: a nurse starts an OPD visit, takes its triage task, completes it, and the visit moves on
// to the consultation worklist. Keyboard only, with axe on the screen.
test('a visit moves from the triage worklist to the consultation worklist', async ({ page }) => {
  const visit = `Encounter/${randomUUID()}`
  await signIn(page, USERS.nurse)
  await press(page, 'Worklists', 'link')
  await expect(page.getByRole('heading', { name: 'Worklists', level: 1 })).toBeVisible()
  await noViolations(page)

  await page.getByLabel('Workflow', { exact: true }).selectOption('https://health.go.ug/fhir/Library/opd-visit|1.0.0')
  await page.getByLabel(/About/).fill(visit)
  await press(page, 'Start')
  await expect(page.getByRole('status').filter({ hasText: 'Started.' })).toBeVisible()

  await page.getByLabel('Worklist', { exact: true }).selectOption('opd-triage')
  const row = page.getByRole('row').filter({ hasText: visit })
  await expect(row).toContainText('Waiting')
  // The only task: chosen with the arrow keys and Enter.
  await page.getByRole('grid').getByRole('columnheader').first().focus()
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('Enter')
  await press(page, 'Take this task')
  await expect(row).toContainText('You have it')
  await noViolations(page)

  await press(page, 'Done: triaged')
  await expect(page.getByRole('row').filter({ hasText: visit })).toHaveCount(0)

  await page.getByLabel('Worklist', { exact: true }).selectOption('opd-consult')
  const consult = page.getByRole('row').filter({ hasText: visit })
  await expect(consult).toContainText('urgent')
  await expect(consult).toContainText('Waiting')
})
