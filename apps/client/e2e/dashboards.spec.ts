import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #602: saved Visualizations and Dashboards at central. e2e-district (Kampala) holds
// analytics.author; e2e-wakiso (Wakiso) may explore but not author. The sample summaries are in
// e2e/summaries.ts: Kampala's KAM013 and KAM020 and Wakiso's WAK001 have synced.
test.describe.serial('saved Dashboards', () => {
  const title = `District ANC ${Date.now()}`
  const visits = 'ANC 1st visits (sample)'

  test('a district user saves a Visualization onto a new Dashboard and shares it with every district', async ({ page }) => {
    await signIn(page, USERS.district)
    await press(page, 'Data explorer', 'link')
    const pickers = page.getByRole('form', { name: 'What to show' })
    await pickers.getByRole('checkbox', { name: 'ANC 1st visit before 12 weeks (sample)' }).uncheck()
    await pickers.getByRole('checkbox', { name: visits }).check()
    await pickers.getByRole('button', { name: 'Each area under Kampala' }).click()
    await pickers.getByRole('group', { name: 'Series' }).getByRole('button', { name: 'Places' }).click()
    await expect(pickers.getByText('Follows whoever views it: each area under their place.')).toBeVisible()

    await page.getByRole('button', { name: 'Save Visualization' }).click()
    const save = page.getByRole('form', { name: 'Save Visualization' })
    await save.getByLabel('Title').fill('ANC 1st visits by area')
    await save.getByLabel('Dashboard').selectOption({ label: 'New Dashboard…' })
    await save.getByLabel('New Dashboard’s title').fill(title)
    await noViolations(page)
    await save.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(page.getByRole('status').filter({ hasText: `Visualization saved on ${title}.` })).toBeVisible()

    await page.getByRole('status').filter({ hasText: 'Visualization saved' }).getByRole('link', { name: 'Dashboards' }).click()
    await expect(page.getByRole('heading', { name: 'Dashboards', level: 1 })).toBeVisible()
    await page.getByRole('navigation', { name: 'Dashboards' }).getByRole('button', { name: new RegExp(title) }).click()
    const sharing = page.getByRole('form', { name: 'Sharing' })
    await expect(sharing.getByText('Not shared: only you see it.')).toBeVisible()
    await sharing.getByRole('checkbox', { name: 'Every district' }).check()
    await sharing.getByRole('button', { name: 'Save sharing' }).click()
    await expect(sharing.getByRole('status')).toHaveText('Sharing saved.')
    await expect(sharing.getByText(/Shared with Every district\./)).toBeVisible()
    const card = page.getByRole('article', { name: 'ANC 1st visits by area' })
    await expect(card.getByRole('table', { name: 'Figures' }).getByRole('rowheader')).toHaveText(['Kisenyi HC IV', 'Kawempe HC III', 'Kitebi HC III'])
    await noViolations(page)
  })

  test('another district’s user sees it with their own district’s data, cannot save, and opens it in the explorer', async ({ page }) => {
    await signIn(page, USERS.wakiso)
    await press(page, 'Dashboards', 'link')
    const list = page.getByRole('navigation', { name: 'Dashboards' })
    await list.getByRole('button', { name: new RegExp(title) }).click()
    await expect(list.getByRole('button', { name: new RegExp(title) })).toContainText('Shared with you')
    // Only its owner shares it.
    await expect(page.getByRole('form', { name: 'Sharing' })).toHaveCount(0)
    await expect(list.getByRole('form', { name: 'New Dashboard' })).toHaveCount(0)
    const card = page.getByRole('article', { name: 'ANC 1st visits by area' })
    const rows = card.getByRole('table', { name: 'Figures' }).getByRole('rowheader')
    await expect(rows.first()).toHaveText('Wakiso HC IV')
    await expect(rows.filter({ hasText: 'Kisenyi HC IV' })).toHaveCount(0)
    await noViolations(page)

    await card.getByRole('link', { name: 'Open in explorer' }).click()
    await expect(page.getByRole('heading', { name: 'Data explorer', level: 1 })).toBeVisible()
    const pickers = page.getByRole('form', { name: 'What to show' })
    await expect(pickers.getByRole('checkbox', { name: visits })).toBeChecked()
    await expect(pickers.getByRole('checkbox', { name: 'ANC 1st visit before 12 weeks (sample)' })).not.toBeChecked()
    await expect(pickers.getByRole('checkbox', { name: 'Wakiso HC IV' })).toBeChecked()
    await expect(pickers.getByRole('checkbox', { name: 'Wakiso', exact: true })).not.toBeChecked()
    await expect(pickers.getByRole('group', { name: 'Series' }).getByRole('button', { name: 'Places' })).toHaveAttribute('aria-pressed', 'true')
    // Without analytics.author there is nothing to save with.
    await expect(page.getByRole('article', { name: 'Visualization' }).getByRole('table', { name: 'Figures' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Save Visualization' })).toHaveCount(0)
  })
})
