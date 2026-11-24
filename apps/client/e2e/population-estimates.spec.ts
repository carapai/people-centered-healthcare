import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #553: a year's Population Estimates (ug.population.2026, drafted by the author) are imported
// from the HMIS lead's CSV. Central checks each place code against the facility registry (the
// NHFR sample's places): a row with an unknown code refuses the file, naming the row. A good file
// shows each district's totals, expected pregnancies derived as 5% of the total population, and
// is saved to the draft. The backend also publishes a sample Population package for this year
// (e2e/backend.ts), which the explorer divides by (#553, explorer.spec.ts).

test('a Population CSV is checked against the registry and its district totals saved', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await expect(page.getByRole('article', { name: 'Population estimates' }).getByText('2 years of population estimates')).toBeVisible()
  await press(page, 'Open population estimates', 'link')
  await page.getByRole('grid', { name: 'Population estimates' }).getByRole('row').filter({ hasText: 'ug.population.2026@' }).getByText('Population estimates 2026', { exact: true }).click()

  await expect(page.getByRole('heading', { name: 'Population estimates 2026', level: 1 })).toBeVisible()
  const types = page.getByRole('table', { name: 'Estimate types' })
  await expect(types.getByRole('row').filter({ hasText: 'Expected pregnancies' }).getByText('5% of Total population')).toBeVisible()
  await expect(page.getByText('4 figures for 4 places')).toBeVisible()
  await noViolations(page)

  const csv = page.getByLabel('Or paste the CSV')
  await csv.fill('place,total-population\nKAM013,40000\nKAM999,1000\n')
  await press(page, 'Check against the registry')
  await expect(page.getByText('row 3: the place KAM999 is not in the facility registry or the NHFR')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Save these figures to the draft' })).toHaveCount(0)

  await csv.fill('place,total-population\nKAM013,40000\nKAM020,60000\ndistrict-wakiso,"3,400,000"\n')
  await press(page, 'Check against the registry')
  const districts = page.getByRole('table', { name: /District totals/ })
  const kampala = districts.getByRole('row').filter({ hasText: 'Kampala' })
  await expect(kampala.getByRole('cell').nth(0)).toHaveText('100,000')
  await expect(kampala.getByRole('cell').nth(1)).toHaveText('5,000')
  await expect(districts.getByRole('row').filter({ hasText: 'Wakiso' }).getByRole('cell').nth(1)).toHaveText('170,000')
  await expect(districts.getByRole('row').filter({ hasText: 'Jinja' })).toHaveCount(0)
  await noViolations(page)
  await press(page, 'Save these figures to the draft')
  await expect(page.getByText('Saved.')).toBeVisible()
  // Opened again, the draft holds the imported figures.
  await page.goBack()
  await page.getByRole('grid', { name: 'Population estimates' }).getByRole('row').filter({ hasText: 'ug.population.2026@' }).getByText('Population estimates 2026', { exact: true }).click()
  await expect(page.getByText('3 figures for 3 places')).toBeVisible()
})
