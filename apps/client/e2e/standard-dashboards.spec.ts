import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #603: standard Dashboards ship in a Reporting package. e2e/backend.ts publishes the sample one
// (e2e/fixtures/dashboards, ug.moh.sample-dashboards@1.0.0) with the ANC overview over the sample
// indicators; the sample summaries (e2e/summaries.ts) cover Kampala's KAM013 and KAM020 and
// Wakiso's WAK001.
test.describe.serial('standard Dashboards', () => {
  test('a published Reporting package adds ANC overview to every user’s Dashboards tab, over their own area', async ({ page }) => {
    await signIn(page, USERS.wakiso)
    await press(page, 'Dashboards', 'link')
    const list = page.getByRole('navigation', { name: 'Dashboards' })
    const overview = list.getByRole('button', { name: /^ANC overview/ })
    await expect(overview).toContainText('From ug.moh.sample-dashboards@1.0.0')
    await overview.click()
    await expect(page.getByText(/changes only in a new version of its Reporting package/)).toBeVisible()
    const card = page.getByRole('article', { name: 'ANC 1st visits by area' })
    const rows = card.getByRole('table', { name: 'Figures' }).getByRole('rowheader')
    await expect(rows.first()).toHaveText('Wakiso HC IV')
    await expect(rows.filter({ hasText: 'Kisenyi HC IV' })).toHaveCount(0)
    // It cannot be edited in place, and without analytics.author there is no Save as.
    await expect(page.getByRole('form', { name: 'Sharing' })).toHaveCount(0)
    await expect(page.getByRole('form', { name: 'Save as' })).toHaveCount(0)
    await noViolations(page)
  })

  test('Save as makes a saved copy of a standard Dashboard', async ({ page }) => {
    const title = `My ANC overview ${Date.now()}`
    await signIn(page, USERS.district)
    await press(page, 'Dashboards', 'link')
    const list = page.getByRole('navigation', { name: 'Dashboards' })
    await list.getByRole('button', { name: /^ANC overview/ }).click()
    const card = page.getByRole('article', { name: 'ANC 1st visits by area' })
    await expect(card.getByRole('table', { name: 'Figures' }).getByRole('rowheader')).toHaveText(['Kisenyi HC IV', 'Kawempe HC III', 'Kitebi HC III'])
    await expect(page.getByRole('form', { name: 'Sharing' })).toHaveCount(0)

    const saveAs = page.getByRole('form', { name: 'Save as' })
    await expect(saveAs.getByLabel('Your copy’s title')).toHaveValue('ANC overview (copy)')
    await saveAs.getByLabel('Your copy’s title').fill(title)
    await noViolations(page)
    await saveAs.getByRole('button', { name: 'Save as my Dashboard' }).click()

    // The copy is the user's own: picked, shareable, with the same Visualizations.
    const copy = list.getByRole('button', { name: new RegExp(title) })
    await expect(copy).toContainText('Yours')
    await expect(copy).toHaveAttribute('aria-pressed', 'true')
    await expect(page.getByRole('form', { name: 'Sharing' })).toBeVisible()
    await expect(page.getByRole('article', { name: 'ANC 1st visits by area' }).getByRole('table', { name: 'Figures' }).getByRole('rowheader')).toHaveText(['Kisenyi HC IV', 'Kawempe HC III', 'Kitebi HC III'])
    await expect(page.getByRole('article', { name: 'ANC 1st visit before 12 weeks' })).toBeVisible()
    // The standard one is unchanged.
    await expect(list.getByRole('button', { name: /^ANC overview/ }).first()).toContainText('From ug.moh.sample-dashboards@1.0.0')
  })
})
