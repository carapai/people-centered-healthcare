import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #604: map Visualizations over the national core's district and region boundaries
// (packages/reference-packages/core, published by the backend) and the NHFR sample's facility
// coordinates. This quarter the sample malaria test positivity is 45% in the Central region
// (bad), 25% at Jinja (Eastern, warning) and 10% at Gulu (Northern, good); the Western region
// has no facility in the sample, and KAM031 has never synced (e2e/summaries.ts).
const positivity = 'Malaria test positivity (sample)'

test("a national user's malaria map shades each region by band, and an unsynced facility shows hollow", async ({ page }) => {
  await signIn(page, USERS.national)
  await press(page, 'Data explorer', 'link')
  const pickers = page.getByRole('form', { name: 'What to show' })
  const card = page.getByRole('article', { name: 'Visualization' })
  await pickers.getByRole('checkbox', { name: 'ANC 1st visit before 12 weeks (sample)' }).uncheck()
  await pickers.getByRole('checkbox', { name: positivity }).check()
  await pickers.getByLabel('Period type').selectOption({ label: 'Quarters' })
  const quarters = await pickers.getByLabel('From').getByRole('option').allTextContents()
  const current = quarters.at(-1) ?? ''
  await pickers.getByLabel('From').selectOption(current)
  await pickers.getByRole('button', { name: 'Each area under Ministry of Health' }).click()
  await card.getByRole('group', { name: 'Chart type' }).getByRole('button', { name: 'Map' }).click()

  const map = card.getByRole('img', { name: `Map of ${positivity}, ${current}` })
  await expect(map).toBeVisible()
  const region = (code: string) => map.locator(`path[data-place="${code}"]`)
  await expect(region('region-central')).toHaveAttribute('data-tone', 'bad')
  await expect(region('region-eastern')).toHaveAttribute('data-tone', 'warning')
  await expect(region('region-northern')).toHaveAttribute('data-tone', 'good')
  await expect(region('region-western')).not.toHaveAttribute('data-tone')
  await expect(region('region-central').locator('title')).toHaveText('Central: 45%')
  await expect(region('region-western').locator('title')).toHaveText('Western: no figure')

  // Kitebi HC III has never synced: hollow, saying so on hover; Kisenyi HC IV has.
  const kitebi = map.locator('circle[data-place="KAM031"]')
  await expect(kitebi).toHaveAttribute('data-hollow', 'true')
  await expect(kitebi.locator('title')).toHaveText('Kitebi HC III (never synced)')
  await expect(map.locator('circle[data-place="KAM013"]')).not.toHaveAttribute('data-hollow')
  await expect(card.getByText('A hollow point is a facility that has not synced this period.')).toBeVisible()
  // The figures stay readable as a table under the map.
  await expect(card.getByRole('table', { name: 'Figures' }).getByRole('row', { name: /Northern/ }).getByRole('cell')).toHaveText('10')
  await noViolations(page)
})

test('a district user maps only their district and its facilities', async ({ page }) => {
  await signIn(page, USERS.district)
  await press(page, 'Data explorer', 'link')
  const card = page.getByRole('article', { name: 'Visualization' })
  await card.getByRole('group', { name: 'Chart type' }).getByRole('button', { name: 'Map' }).click()
  const map = card.getByRole('img', { name: /^Map of / }).first()
  await expect(map).toBeVisible()
  await expect(map.locator('path[data-place="district-kampala"]')).toHaveCount(1)
  await expect(map.locator('path[data-place="district-wakiso"]')).toHaveCount(0)
  await expect(map.locator('circle[data-place="KAM013"]')).toHaveCount(1)
  await expect(map.locator('circle[data-place="WAK001"]')).toHaveCount(0)
  await expect(map.locator('circle[data-place="KAM031"]')).toHaveAttribute('data-hollow', 'true')
})
