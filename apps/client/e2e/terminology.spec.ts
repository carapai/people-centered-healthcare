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
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'CodeSystem-triage-category.json in the terminology browser' }).click()
  await expect(page.getByRole('heading', { name: 'Terminology: TriageCategory', level: 1 })).toBeVisible()
  const systems = page.getByRole('navigation', { name: 'Code systems' })
  await expect(systems.getByRole('button', { name: /TriageCategory 1\.0\.0 · 3 concepts/ })).toHaveAttribute('aria-pressed', 'true')
  await expect(systems.getByText('Local · In this package')).toBeVisible()
  const concepts = page.getByRole('region', { name: 'Concepts of TriageCategory' })
  await expect(concepts.getByRole('button', { name: /^emergency/ })).toBeVisible()
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

// #524: the OPD draft's author edits its triage CodeSystem in the terminology browser: a code
// added under another, with a note for reviewers, shows in the tree, the Version panel and What
// it reaches; being new, it is renamed and then deleted, and the note is in the draft's
// discussion. A published code being fixed, retired, and the minor and major bumps need a
// published version, which the seeded backend has none of: central-server's studio tests cover
// them.
test('the author adds a code under another to the triage code system, renames it and deletes it', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'Open the packages', 'link')
  await page.getByRole('grid', { name: 'Packages' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'CodeSystem-triage-category.json in the terminology browser' }).click()
  const concepts = page.getByRole('region', { name: 'Concepts of TriageCategory' })
  await concepts.getByRole('button', { name: '+ under urgent' }).click()
  const adding = concepts.getByRole('form', { name: 'New code under urgent' })
  await adding.getByLabel('Code', { exact: true }).fill('urgent-pain')
  await adding.getByLabel('Display', { exact: true }).fill('Urgent: severe pain')
  await adding.getByLabel('Note for reviewers').fill('Pain protocol 2026')
  await adding.getByRole('button', { name: 'Add the code' }).click()
  const edit = concepts.getByRole('form', { name: 'Code urgent-pain' })
  await expect(edit).toBeVisible()
  await expect(concepts.getByRole('button', { name: /^urgent-pain/ })).toBeVisible()
  await expect(concepts.getByRole('region', { name: 'Version' }).getByText('Never published: it keeps its version 1.0.0.')).toBeVisible()
  await expect(concepts.getByRole('region', { name: 'What it reaches' }).getByText(/Gains .*urgent-pain/)).toBeVisible()
  await noViolations(page)

  // Being new, it is renamed, then deleted.
  await edit.getByLabel('Code', { exact: true }).fill('urgent-severe-pain')
  await edit.getByRole('button', { name: 'Save the code' }).click()
  const renamed = concepts.getByRole('form', { name: 'Code urgent-severe-pain' })
  await expect(renamed).toBeVisible()
  await renamed.getByRole('button', { name: 'Delete urgent-severe-pain' }).click()
  await expect(concepts.getByRole('button', { name: /^urgent-severe-pain/ })).toBeHidden()

  // Reviewers read the note in the draft's discussion.
  await press(page, 'Back to the package', 'link')
  await expect(page.getByRole('region', { name: 'Discussion' }).getByText('Added code urgent-pain under urgent: Pain protocol 2026')).toBeVisible()
})
