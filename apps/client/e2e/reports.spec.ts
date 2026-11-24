import { readFileSync } from 'node:fs'

import { expect, test } from '@playwright/test'

import { parseCsv } from '../src/features/reports/csv'
import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #557: running a Report at central. e2e/backend.ts publishes the sample Reporting package
// (e2e/fixtures/dashboards) with the sample ANC report over the sample indicators, whose
// summaries (e2e/summaries.ts) KAM013 and KAM020 synced; KAM031 never has, so it is missing.
test('a district user runs a Report for Kampala, prints it and exports the same cells as CSV', async ({ page }) => {
  await signIn(page, USERS.district)
  await press(page, 'Reports', 'link')
  const list = page.getByRole('list', { name: 'Reports' })
  await expect(list.getByRole('button', { name: /^Sample ANC report/ })).toContainText('From ug.moh.sample-dashboards@1.0.0')
  await list.getByRole('button', { name: /^Sample ANC report/ }).click()
  // Only places in Kampala are offered, never Wakiso's.
  const places = page.getByLabel('Place')
  await expect(places.getByRole('option')).toHaveText(['Kampala', 'Kisenyi HC IV', 'Kawempe HC III', 'Kitebi HC III'])

  const report = page.getByRole('article', { name: 'Sample ANC report' })
  await expect(report.getByText('2 of 3 facilities reported.')).toBeVisible()
  await expect(report.getByText(/Kitebi HC III: missing, never synced/)).toBeVisible()
  // The Total is the whole, rolled up from summed parts: it is the sum of the age bands.
  const attendance = report.getByRole('table', { name: 'ANC attendance' })
  await expect(attendance.getByRole('columnheader')).toHaveText(['Indicator', '10–14 years', '15–19 years', '20–24 years', '25+ years', 'Not recorded', 'Total', '% change'])
  const cells = await attendance.getByRole('row', { name: /ANC 1st visits/ }).getByRole('cell').allTextContents()
  const bands = cells.slice(0, 5).map(Number)
  expect(bands.reduce((a, b) => a + b, 0)).toBe(Number(cells[5]))
  await expect(report.getByRole('table', { name: 'Timing of the 1st visit' }).getByRole('cell').first()).toHaveText(/%$/)
  await expect(page.getByRole('button', { name: 'Print or save as PDF' })).toBeVisible()
  await noViolations(page)

  // The CSV holds exactly the cells on screen.
  const [download] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: 'Download CSV' }).click()])
  const csv = parseCsv(readFileSync((await download.path()) ?? '', 'utf8'))
  const screen: string[][] = []
  for (const name of ['ANC attendance', 'Timing of the 1st visit']) {
    const table = report.getByRole('table', { name })
    screen.push([''], [name], await table.getByRole('columnheader').allTextContents())
    for (const row of await table.getByRole('row').all()) {
      const head = await row.getByRole('rowheader').allTextContents()
      if (head.length === 0) continue
      screen.push([...head, ...(await row.getByRole('cell').allTextContents())])
    }
  }
  expect(csv.slice(2)).toEqual(screen)

  // A facility of the area runs too.
  await places.selectOption({ label: 'Kisenyi HC IV' })
  await expect(report.getByText('1 of 1 facility reported.')).toBeVisible()
})
