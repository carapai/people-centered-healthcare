import { expect, test, type Page } from '@playwright/test'

import { noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// #855: the Studio starts from what is designed: its home, a section, New and Edit. These tests
// start drafts of the published clinic package and of a new one.

/** Opens a section from the home: the shell's Studio link, or an editor's way back to it. */
async function openSection(page: Page, name: string) {
  await page.getByRole('link', { name: /^(Studio|Back to the Studio)$/ }).first().click()
  await expect(page.getByRole('heading', { name: 'Studio', level: 1 })).toBeVisible()
  await press(page, `Open ${name}`, 'link')
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
}

test('the Studio home shows the design sections, and Forms lists the OPD form', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  for (const section of ['Forms', 'Workflows', 'Concepts & value sets', 'Decision tables', 'Age band sets', 'Indicators', 'Reports', 'Dashboards', 'Packages'])
    await expect(page.getByRole('heading', { name: section, level: 2, exact: true })).toBeVisible()
  await expect(page.getByRole('article', { name: 'Dashboards' }).getByText('Coming soon')).toBeVisible()
  await expect(page.getByRole('article', { name: 'Reports' }).getByText(/\d+ reports?/)).toBeVisible()
  await expect(page.getByRole('article', { name: 'Indicators' }).getByText(/\d+ indicators?/)).toBeVisible()
  await expect(page.getByRole('article', { name: 'Forms' }).getByText(/\d+ forms?/)).toBeVisible()
  await noViolations(page)

  await press(page, 'Open forms', 'link')
  await expect(page.getByRole('heading', { name: 'Forms', level: 1 })).toBeVisible()
  const grid = page.getByRole('grid', { name: 'Forms' })
  await expect(grid.getByText('OPD triage')).toBeVisible()
  await expect(grid.getByRole('row').filter({ hasText: 'OPD triage' }).getByText('In your draft')).toBeVisible()
  await noViolations(page)
  // Search and the status filter narrow it.
  await type(page, 'Search', 'no such form')
  await expect(page.getByText('Nothing matches the search and filters.')).toBeVisible()
  await page.getByLabel('Search').fill('triage')
  await expect(grid.getByText('OPD triage')).toBeVisible()
  await page.getByLabel('Status').selectOption('published')
  await expect(page.getByText('Nothing matches the search and filters.')).toBeVisible()
  await page.getByLabel('Status').selectOption('')

  // A row opens the form designer, with the package and its draft page above it.
  await grid.getByText('OPD triage').click()
  await expect(page.getByRole('heading', { name: 'Form designer: OPD triage', level: 1 })).toBeVisible()
  const bar = page.getByRole('navigation', { name: 'Where this design lives' })
  await expect(bar.getByText(/Package .*\(ug\.moh\.opd\), version 1\.0\.0: draft/)).toBeVisible()
  await expect(bar.getByRole('link', { name: 'Open the draft page' })).toBeVisible()
})

test('Edit on a published workflow starts a draft and opens it editable; a second Edit reuses it', async ({ page }) => {
  await signIn(page, USERS.author)
  await openSection(page, 'workflows')
  const grid = page.getByRole('grid', { name: 'Workflows' })
  await expect(grid.getByRole('row').filter({ hasText: 'Outreach visit' }).getByText('Published')).toBeVisible()
  await grid.getByText('Outreach visit').click()
  await expect(page.getByRole('heading', { name: 'Workflow Studio: Outreach visit', level: 1 })).toBeVisible()
  const published = page.url()
  expect(published).toContain('ug.moh.clinic%401.0.0')
  await expect(page.getByText(/You cannot change this package/)).toBeVisible()
  await expect(page.getByRole('button', { name: 'Save' })).toHaveCount(0)
  await noViolations(page)

  await press(page, 'Edit')
  await page.waitForURL(/ug\.moh\.clinic%401\.1\.0-draft/)
  await expect(page.getByRole('heading', { name: 'Workflow Studio: Outreach visit', level: 1 })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Save' })).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Where this design lives' }).getByText(/version 1\.1\.0-draft: draft/)).toBeVisible()
  const drafted = page.url()

  // The published version again: Edit now reopens the same draft instead of starting another.
  await page.goto(published)
  await page.waitForURL(/\/sign-in\?/)
  await type(page, 'Username', USERS.author.username)
  await page.keyboard.press('Tab')
  await page.keyboard.type(USERS.author.password)
  await page.keyboard.press('Enter')
  await expect(page.getByText('Opens it in your draft ug.moh.clinic@1.1.0-draft.')).toBeVisible()
  await press(page, 'Edit')
  await page.waitForURL(drafted)
  await expect(page.getByRole('button', { name: 'Save' })).toBeVisible()
})

test('New form in a new package starts its draft and opens the form designer', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await press(page, 'New form', 'link')
  await expect(page.getByRole('heading', { name: 'A new form' })).toBeVisible()
  await page.getByRole('radio', { name: 'A new package' }).check()
  await page.getByLabel(/Package name/).fill('ug.moh.screening')
  await page.getByLabel('Package title').fill('Community screening')
  await page.getByLabel(/Canonical base URL/).fill('https://health.go.ug/fhir')
  await page.getByLabel('Title', { exact: true }).fill('Screening visit')
  await noViolations(page)
  await press(page, 'Create and open')
  await expect(page.getByRole('heading', { name: 'Form designer: Screening visit', level: 1 })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Save' })).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Where this design lives' }).getByText(/\(ug\.moh\.screening\), version 0\.1\.0: draft/)).toBeVisible()

  await openSection(page, 'forms')
  await expect(page.getByRole('grid', { name: 'Forms' }).getByText('Screening visit')).toBeVisible()
})

test('Concepts & value sets and Decision tables list and create the same way', async ({ page }) => {
  await signIn(page, USERS.author)
  await openSection(page, 'concepts & value sets')
  await expect(page.getByRole('grid', { name: 'Concepts & value sets' }).getByText('TriageCategory')).toBeVisible()
  await press(page, 'New code system or value set')
  await page.getByRole('radio', { name: /Outreach clinic: your draft/ }).check()
  await page.getByLabel('What to create').selectOption('code-system')
  await page.getByLabel('Title', { exact: true }).fill('Outreach reasons')
  await press(page, 'Create and open')
  await expect(page.getByRole('heading', { name: /Terminology: Outreach reasons/, level: 1 })).toBeVisible()

  await openSection(page, 'decision tables')
  await press(page, 'New decision table')
  await page.getByRole('radio', { name: /Outreach clinic: your draft/ }).check()
  await expect(page.getByLabel('In which workflow')).toHaveValue('Library-outreach-visit.json')
  await page.getByLabel('Title', { exact: true }).fill('Follow up')
  await noViolations(page)
  await press(page, 'Create and open')
  await expect(page.getByRole('heading', { name: 'Decision table follow-up' })).toBeVisible()

  await openSection(page, 'decision tables')
  const row = page.getByRole('grid', { name: 'Decision tables' }).getByRole('row').filter({ hasText: 'follow-up' })
  await expect(row.getByText('In your draft')).toBeVisible()
})

// #556: Reports in the HMIS Reporting package (ug.moh.hmis), the author's draft, are listed and
// open in the file editor until the Report designer (#559).
test('Reports lists the HMIS package’s Reports and opens one in the file editor', async ({ page }) => {
  await signIn(page, USERS.author)
  await openSection(page, 'reports')
  await expect(page.getByRole('heading', { name: 'Reports', level: 1 })).toBeVisible()
  const grid = page.getByRole('grid', { name: 'Reports' })
  await expect(grid.getByText('ANC facility monitoring')).toBeVisible()
  await expect(grid.getByRole('row').filter({ hasText: 'HMIS 105 · 2 Antenatal care' }).getByText('In your draft')).toBeVisible()
  await expect(page.getByRole('button', { name: 'New report' })).toHaveCount(0)
  await noViolations(page)
  await grid.getByText('HMIS 105 · 2 Antenatal care').click()
  await expect(page.getByRole('heading', { name: 'configuration/reports/hmis-105-anc.json in ug.moh.hmis@1.0.0', level: 1 })).toBeVisible()
})
