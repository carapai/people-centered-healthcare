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

// #600: line, table and single-number Visualizations, coloured by threshold bands. The sample
// malaria test positivity (e2e/fixtures/indicators) is 45% at every facility this quarter, in
// its *bad* band (40% and above), and 30% the quarter before (e2e/summaries.ts).
test('switching chart type keeps the selection, a single number shows its change, and 45% positivity is bad', async ({ page }) => {
  await signIn(page, USERS.district)
  await press(page, 'Data explorer', 'link')
  const pickers = page.getByRole('form', { name: 'What to show' })
  const card = page.getByRole('article', { name: 'Visualization' })
  const positivity = 'Malaria test positivity (sample)'
  await pickers.getByRole('checkbox', { name: 'ANC 1st visit before 12 weeks (sample)' }).uncheck()
  await pickers.getByRole('checkbox', { name: positivity }).check()
  await pickers.getByLabel('Period type').selectOption({ label: 'Quarters' })
  // This quarter alone: the single number asks for the quarter before itself.
  const quarters = await pickers.getByLabel('From').getByRole('option').allTextContents()
  const [previous, current] = quarters.slice(-2)
  await pickers.getByLabel('From').selectOption(current ?? '')

  const types = card.getByRole('group', { name: 'Chart type' })
  await types.getByRole('button', { name: 'Single number' }).click()
  const tile = card.getByRole('list', { name: 'Single numbers' }).getByRole('listitem', { name: positivity })
  await expect(tile).toContainText('45%')
  await expect(tile).toHaveAttribute('data-tone', 'bad')
  await expect(tile).toContainText('Bad')
  await expect(tile).toContainText(`+15 on ${previous} (30%)`)
  const legend = card.getByRole('list', { name: `${positivity} thresholds` })
  await expect(legend.getByRole('listitem')).toHaveText(['below 20: Good', '20 to under 40: Warning', '40 and above: Bad'])
  await noViolations(page)

  // The selection is kept through every type.
  await types.getByRole('button', { name: 'Line' }).click()
  await expect(types.getByRole('button', { name: 'Line' })).toHaveAttribute('aria-pressed', 'true')
  await expect(card.getByRole('img', { name: `Line chart of ${positivity}, ${current} to ${current}` })).toBeVisible()
  await types.getByRole('button', { name: 'Table' }).click()
  await expect(pickers.getByRole('checkbox', { name: positivity })).toBeChecked()
  await expect(pickers.getByLabel('Period type')).toHaveValue('quarterly')
  await expect(pickers.getByLabel('From')).toHaveValue(current ?? '')
  await expect(pickers.getByRole('checkbox', { name: 'Kampala' })).toBeChecked()
  const table = card.getByRole('table', { name: 'Figures' })
  await expect(table.getByRole('columnheader', { name: positivity })).toBeVisible()
  const kampala = table.getByRole('row', { name: /Kampala/ }).getByRole('cell')
  await expect(kampala).toHaveText('45')
  await expect(kampala).toHaveAttribute('data-tone', 'bad')
  await card.getByRole('group', { name: 'Table layout' }).getByRole('button', { name: 'Indicators × periods' }).click()
  await expect(table.getByRole('columnheader', { name: current ?? '' })).toBeVisible()
  await expect(legend).toBeVisible()
  await noViolations(page)
})

// #601: disaggregation and series. The sample ANC 1st visits are split by MoH's ANC age bands
// at every facility (e2e/summaries.ts), and Kampala's bands are rolled up from their summed parts.
test('ANC 1st visits by age band show a series per band, and series by place compare the areas under Kampala', async ({ page }) => {
  await signIn(page, USERS.district)
  await press(page, 'Data explorer', 'link')
  const pickers = page.getByRole('form', { name: 'What to show' })
  const card = page.getByRole('article', { name: 'Visualization' })
  const visits = 'ANC 1st visits (sample)'
  await pickers.getByRole('checkbox', { name: 'ANC 1st visit before 12 weeks (sample)' }).uncheck()
  await pickers.getByRole('checkbox', { name: visits }).check()
  const series = pickers.getByRole('group', { name: 'Series' })
  // No disaggregation picked: no band series.
  await expect(series.getByRole('button', { name: 'Bands' })).toBeDisabled()
  await pickers.getByRole('combobox', { name: 'Disaggregation' }).selectOption('Age band')
  await expect(series.getByRole('button', { name: 'Bands' })).toHaveAttribute('aria-pressed', 'true')
  const key = card.locator('figcaption')
  await expect(key).toContainText('15-19')
  await expect(key).toContainText('25+')
  await expect(key).toContainText('Not recorded')
  const table = card.getByRole('table', { name: 'Figures' })
  await expect(table.getByRole('rowheader')).toHaveText(['10-14', '15-19', '20-24', '25+', 'Not recorded'])
  await noViolations(page)

  // A median has no age bands: picking it drops the disaggregation.
  await pickers.getByRole('checkbox', { name: 'Median gestational age at ANC 1st visit (sample)' }).check()
  await expect(pickers.getByRole('combobox', { name: 'Disaggregation' })).toHaveValue('')
  await pickers.getByRole('checkbox', { name: 'Median gestational age at ANC 1st visit (sample)' }).uncheck()

  // Series by place: Kampala's facilities over the months, each its own series.
  await pickers.getByRole('button', { name: 'Each area under Kampala' }).click()
  await series.getByRole('button', { name: 'Places' }).click()
  await expect(pickers.getByRole('checkbox', { name: 'Kampala' })).not.toBeChecked()
  await expect(table.getByRole('rowheader')).toHaveText(['Kisenyi HC IV', 'Kawempe HC III', 'Kitebi HC III'])
  await noViolations(page)
})
