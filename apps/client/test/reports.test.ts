import { describe, expect, test } from 'vitest'

import hmis105 from '../../../packages/reference-packages/hmis/configuration/reports/hmis-105-anc.json'
import { type GridTexts, type ReportRun, closedPeriods, periodOf, reportCsv, reportsQuery, runGrids, runQuery } from '../src/features/reports/api'
import { parseCsv } from '../src/features/reports/csv'
import { readReport, type Report } from '../src/features/studio/reports/report'

// Running a Report (#557): the run laid out as the screen shows it, the CSV export holding the
// same cells, totals taken from the run's whole (summed parts at the server), and the periods
// offered.

const BANDS = 'https://health.go.ug/fhir/Library/age-bands-anc|1.0.0'
const read = readReport(JSON.stringify(hmis105))
if (!('report' in read)) throw new Error(read.refusal)
const report: Report = read.report

const texts: GridTexts = {
  indicator: 'Indicator',
  total: 'Total',
  value: 'Value',
  change: '% change',
  notShown: (cell) => `n/a (${cell.notShown ?? ''})`,
  figure: (value, calculation) => (calculation === 'percentage' ? `${value}%` : value),
}

const bands = (counts: number[]) =>
  ['10-14', '15-19', '20-24', '25+', 'not-recorded'].map((code, n) => ({ code, label: code === 'not-recorded' ? 'Not recorded' : `${code} years`, value: String(counts[n] ?? 0), count: counts[n] ?? 0, denominator: null }))

const counted = (counts: number[]) => {
  const total = counts.reduce((a, b) => a + b, 0)
  return { value: String(total), count: total, denominator: null, disaggregations: [{ name: BANDS, bands: bands(counts) }] }
}

/** Kampala's March: first visits by age band, and two percentages whose wholes are summed parts (40 of 60), never averages. */
const run: ReportRun = {
  where: 'central',
  report: { id: 'ug.moh.hmis:hmis-105-anc', title: 'HMIS 105 · 2 Antenatal care', package: 'ug.moh.hmis@1.0.0' },
  place: { code: 'district-kampala', name: 'Kampala', level: 'district' },
  period: { id: '2026-03', start: '2026-03-01', end: '2026-03-31' },
  previous: { id: '2026-02', start: '2026-02-01', end: '2026-02-28' },
  completeness: { reported: 2, expected: 3, computedAt: '2026-04-01T06:00:00Z', missing: [{ facility: 'KAM031', name: 'Kitebi HC III', lastSync: null }] },
  sections: [
    {
      kind: 'table',
      disaggregations: [{ name: BANDS, title: 'ANC age bands', bands: bands([]).map(({ code, label }) => ({ code, label })) }],
      rows: [
        { row: 'first-visits', indicator: { title: 'ANC 1st visits', calculation: 'count' }, current: counted([1, 3, 4, 2, 0]), previous: counted([1, 2, 2, 3, 0]), error: null },
        { row: 'visits', indicator: { title: 'ANC visits', calculation: 'count' }, current: counted([2, 6, 9, 5, 1]), previous: null, error: null },
      ],
    },
    {
      kind: 'table',
      disaggregations: [],
      rows: [
        { row: 'before-12-weeks', indicator: { title: 'Before 12 weeks', calculation: 'percentage' }, current: { value: '66.67', count: 40, denominator: 60, disaggregations: [] }, previous: { value: '50', count: 2, denominator: 4, disaggregations: [] }, error: null },
        { row: 'next-visit-booked', indicator: null, current: null, previous: null, error: 'no indicator is x' },
      ],
    },
  ],
}

describe('a Report run', () => {
  test('lays out each table section: band columns, a Total from the whole, and % change', () => {
    const [attendance, timing] = runGrids(report, run, texts)
    expect(attendance?.grid.header).toEqual(['Indicator', '10-14 years', '15-19 years', '20-24 years', '25+ years', 'Not recorded', 'Total', '% change'])
    expect(attendance?.grid.rows[0]).toEqual({ head: 'ANC 1st visit for women', cells: ['1', '3', '4', '2', '0', '10', '25.0%'] })
    // No previous figure: no change.
    expect(attendance?.grid.rows[1]?.cells.at(-1)).toBe('—')
    // A percentage's whole is the server's, from summed parts (40 of 60), not an average of bands.
    expect(timing?.grid.header).toEqual(['Indicator', 'Value', '% change'])
    expect(timing?.grid.rows[0]).toEqual({ head: 'ANC 1st visit before 12 weeks (%)', cells: ['66.67%', '33.3%'] })
    expect(timing?.grid.rows[1]?.cells[0]).toBe('n/a (no-indicator)')
  })

  test('exports as CSV the same cells as the screen', () => {
    const csv = reportCsv(report, run, texts)
    const rows = parseCsv(csv)
    expect(rows[0]).toEqual(['HMIS 105 · 2 Antenatal care'])
    expect(rows[1]).toEqual(['Kampala', '2026-03'])
    const screen = runGrids(report, run, texts).flatMap(({ section, grid }) => [[''], [section.title], grid.header, ...grid.rows.map((r) => [r.head, ...r.cells])])
    expect(rows.slice(2)).toEqual(screen)
  })

  test('quotes CSV fields that need it', () => {
    const quoted = { ...run, place: { ...run.place, name: 'Kampala, "Central"' } }
    expect(parseCsv(reportCsv(report, quoted, texts))[1]).toEqual(['Kampala, "Central"', '2026-03'])
  })
})

describe('the periods a Report runs for', () => {
  test('are the closed periods of its type, newest first', () => {
    const today = new Date(Date.UTC(2026, 3, 3))
    expect(closedPeriods('monthly', today, 3)).toEqual(['2026-03', '2026-02', '2026-01'])
    expect(closedPeriods('quarterly', today, 2)).toEqual(['2026Q1', '2025Q4'])
    expect(closedPeriods('semiAnnual', today, 2)).toEqual(['2025S2', '2025S1'])
    expect(closedPeriods('annual', today, 1)).toEqual(['2025'])
    expect(closedPeriods('financialJuly', today, 1)).toEqual(['2024July'])
    expect(periodOf('weekly', today)).toBe('2026W14')
  })
})

describe('the Reports API', () => {
  test('lists Reports with their definitions read, and runs one', async () => {
    const asked: { method: string; path: string; body?: unknown }[] = []
    const transport = {
      request: async (r: { method: string; path: string; body?: unknown }) => {
        asked.push(r)
        if (r.path === '/analytics/reports') return { status: 200, headers: {}, body: { where: 'central', reports: [{ id: 'ug.moh.hmis:hmis-105-anc', title: 'HMIS 105', package: 'ug.moh.hmis@1.0.0', periodType: 'monthly', levels: ['district'], definition: { ...hmis105, $schema: undefined }, places: [] }] } }
        return { status: 200, headers: {}, body: run }
      },
    } as never
    const listed = await reportsQuery(transport).queryFn({})
    expect(listed.reports[0]?.definition.sections[0]?.kind).toBe('table')
    const got = await runQuery(transport, { report: 'ug.moh.hmis:hmis-105-anc', place: 'district-kampala', period: '2026-03' }).queryFn({})
    expect(got.completeness.expected).toBe(3)
    expect(asked[1]).toMatchObject({ method: 'POST', path: '/analytics/reports/run', body: { place: 'district-kampala' } })
  })

  test('says why it cannot run', async () => {
    const transport = { request: async () => ({ status: 403, headers: {}, body: { detail: 'needs reporting.view' } }) } as never
    await expect(reportsQuery(transport).queryFn({})).rejects.toMatchObject({ status: 403, detail: 'needs reporting.view' })
  })
})
