import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #599: the data explorer at central, for e2e-district (Kampala). The backend publishes the sample
// indicators (e2e/fixtures/indicators) and receives sample summaries for KAM013, KAM020 and WAK001
// (e2e/summaries.ts); KAM031 never synced.
test('a district user explores their district: a column chart, the missing facility and no median', async ({ page }) => {
  await signIn(page, USERS.district)
  await press(page, 'Data explorer', 'link')
  await expect(page.getByRole('heading', { name: 'Data explorer', level: 1 })).toBeVisible()
  const pickers = page.getByRole('form', { name: 'What to show' })
  // The district and its facilities only: Wakiso is outside the area.
  await expect(pickers.getByRole('checkbox', { name: 'Kampala' })).toBeChecked()
  await expect(pickers.getByRole('checkbox', { name: 'Kisenyi HC IV' })).toBeVisible()
  await expect(pickers.getByRole('checkbox', { name: 'Wakiso HC IV' })).toHaveCount(0)
  // Standard periods only.
  await expect(pickers.getByLabel('Period type').getByRole('option')).toHaveText(['Weeks', 'Months', 'Quarters', 'Financial years (July)'])

  const card = page.getByRole('article', { name: 'Visualization' })
  await expect(card.getByRole('img', { name: /Column chart of ANC 1st visit before 12 weeks \(sample\)/ })).toBeVisible()
  await expect(card.getByText('Rolled up at central from the facilities’ summaries.', { exact: false })).toBeVisible()
  await expect(card.getByText(/Kampala: 2 of 3 facilities reported/)).toBeVisible()
  await expect(card.getByRole('note', { name: 'Missing, not counted as zero:' }).getByText(/Kitebi HC III \(never synced\)/).first()).toBeVisible()
  await noViolations(page)

  // A median above facility level shows the reason instead of a value.
  await pickers.getByRole('checkbox', { name: 'Median gestational age at ANC 1st visit (sample)' }).check()
  await expect(card.getByText(/Median gestational age at ANC 1st visit \(sample\) shows at facility level only/)).toBeVisible()
})
