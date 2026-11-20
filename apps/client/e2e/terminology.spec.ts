import { expect, test } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// #523: the OPD draft's author opens its triage CodeSystem in the terminology browser from the
// draft page, searches it on the server, starts a ValueSet, adds a concept alone and one with
// everything under it, sees them ticked and expanded, saves, and removes one again. With axe.
// Removals flagged against a published expansion need a published version, which the seeded
// backend has none of: central-server's studio tests cover them.
test('the author builds a ValueSet from the triage code system in the terminology browser', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'CodeSystem-triage-category.json in the terminology browser' }).click()
  await expect(page.getByRole('heading', { name: 'Terminology: TriageCategory', level: 1 })).toBeVisible()
  const systems = page.getByRole('navigation', { name: 'Code systems' })
  await expect(systems.getByRole('button', { name: /TriageCategory 1\.0\.0 · 3 concepts/ })).toHaveAttribute('aria-pressed', 'true')
  await expect(systems.getByText('Local · In this package')).toBeVisible()
  const concepts = page.getByRole('region', { name: 'Concepts of TriageCategory' })
  await expect(concepts.getByRole('button', { name: /emergency/ })).toBeVisible()
  await noViolations(page)

  // Search runs on the server.
  await type(page, 'Search by code or words', 'seen next')
  await press(page, 'Search')
  await expect(concepts.getByRole('status').filter({ hasText: '1 match, 1 shown' })).toBeVisible()
  await press(page, 'Back to the hierarchy')

  // A new ValueSet, then a concept alone and one with everything under it.
  const valueSet = page.getByRole('region', { name: 'ValueSet', exact: true })
  await valueSet.getByLabel('Id', { exact: true }).fill('triage-seen-soon')
  await valueSet.getByLabel('Title', { exact: true }).fill('Seen soon')
  await press(page, 'Start the ValueSet')
  await expect(valueSet.getByText('https://health.go.ug/fhir/ValueSet/triage-seen-soon|1.0.0')).toBeVisible()
  await concepts.getByRole('button', { name: /^emergency/ }).click()
  await expect(concepts.getByText('Emergency: seen at once').last()).toBeVisible()
  await press(page, /^Add emergency$/)
  await concepts.getByRole('button', { name: /^urgent/ }).click()
  await press(page, 'Add urgent and everything under it')
  await expect(valueSet.getByRole('heading', { name: 'Expansion: 2 codes' })).toBeVisible()
  await expect(valueSet.getByText('urgent and everything under it, from https://health.go.ug/fhir/CodeSystem/triage-category')).toBeVisible()
  await expect(concepts.getByRole('button', { name: /^emergency.*\(in the ValueSet\)/ })).toBeVisible()
  await expect(valueSet.getByText('Never published: every code is new.')).toBeVisible()
  await noViolations(page)

  // Saved, it opens from its own file.
  await press(page, 'Save the ValueSet')
  await page.waitForURL(/\/terminology\/ValueSet-triage-seen-soon\.json$/)

  // Opened again from its file, it is as saved; a code removed leaves the expansion.
  await expect(page.getByRole('heading', { name: 'Terminology: Seen soon', level: 1 })).toBeVisible()
  await expect(valueSet.getByRole('heading', { name: 'Expansion: 2 codes' })).toBeVisible()
  await press(page, 'Remove emergency')
  await expect(valueSet.getByRole('heading', { name: 'Expansion: 1 code' })).toBeVisible()
  await expect(valueSet.getByText('Unsaved changes')).toBeVisible()
  await press(page, 'Save the ValueSet')
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()
  await expect(valueSet.getByText('Unsaved changes')).toBeHidden()
})
