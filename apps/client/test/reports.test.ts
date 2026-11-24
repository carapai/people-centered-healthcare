import { describe, expect, test } from 'vitest'

import monitoring from '../../../packages/reference-packages/hmis/configuration/reports/anc-facility-monitoring.json'
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
  patient: 'Patient',
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

describe('a Report run on the facility’s edge (#557, part 2)', () => {
  const readMonitoring = readReport(JSON.stringify(monitoring))
  if (!('report' in readMonitoring)) throw new Error(readMonitoring.refusal)
  const watched = readMonitoring.report
  const edgeRun: ReportRun = {
    ...run,
    where: 'facility',
    place: { code: 'KAM013', name: 'Kampala HC IV', level: 'facility' },
    sections: [
      { kind: 'table', disaggregations: [], rows: [{ row: 'median-ga', indicator: { title: 'Median GA', calculation: 'median' }, current: { value: '14', count: 3, denominator: null, disaggregations: [] }, previous: null, error: null }] },
      {
        kind: 'lineList',
        list: {
          program: { title: 'Antenatal care', episodeType: 'ANC' },
          rows: [
            { patient: 'Nakato, Sarah', record: 'EpisodeOfCare/1', cells: [[{ value: 'Nakato, Sarah' }], [{ value: '25' }], [], [{ value: '2026-03-02' }], [{ value: '2026-03-30' }], [{ value: '14', date: '2026-03-30' }]] },
          ],
          error: null,
        },
      },
      { kind: 'lineList', list: { program: null, rows: [], error: 'no indicator is x here' } },
    ],
  }

  test('lays a line list out as the screen shows it, the patient first and an empty cell a dash', () => {
    const grids = runGrids(watched, edgeRun, texts)
    const clients = grids.find((g) => g.section.id === 'clients')
    expect(clients?.grid.header).toEqual(['Patient', 'Name', 'Age (years)', 'Phone', 'Enrolled on', 'Last visit', 'Gestational age (weeks)'])
    expect(clients?.grid.rows).toEqual([{ head: 'Nakato, Sarah', cells: ['Nakato, Sarah', '25', '—', '2026-03-02', '2026-03-30', '14'] }])
    // One that could not run has no cells.
    expect(grids.some((g) => g.section.id === 'first-visits')).toBe(false)
  })

  test('exports its line lists in the CSV as on the screen', () => {
    const rows = parseCsv(reportCsv(watched, edgeRun, texts))
    const screen = runGrids(watched, edgeRun, texts).flatMap(({ section, grid }) => [[''], [section.title], grid.header, ...grid.rows.map((r) => [r.head, ...r.cells])])
    expect(rows.slice(2)).toEqual(screen)
    expect(rows).toContainEqual(['Nakato, Sarah', 'Nakato, Sarah', '25', '—', '2026-03-02', '2026-03-30', '14'])
  })
})

const bandsOf = (name: string, codes: string[]) => ({ name, title: name, bands: codes.map((code) => ({ code, label: code })) })

describe('a crossed cell in a Report run', () => {
  const crossedReport = readReport(
    JSON.stringify({
      ...hmis105,
      sections: [
        {
          kind: 'table',
          id: 'tested',
          title: 'Tested',
          disaggregations: [
            { code: 'hiv', expression: '%record.code', axis: 'columns' },
            { code: 'visit', expression: '%record.type', axis: 'columns' },
          ],
          rows: [{ id: 'tested', indicator: 'https://x/Measure/tested|1.0.0', label: 'Tested' }],
          columnTotals: false,
          rowTotals: false,
          change: false,
          sendToDhis2: false,
        },
      ],
    }),
  )
  if (!('report' in crossedReport)) throw new Error(crossedReport.refusal)
  const crossedRun: ReportRun = {
    ...run,
    sections: [
      {
        kind: 'table',
        disaggregations: [bandsOf('hiv', ['positive', 'negative']), bandsOf('visit', ['first', 'later'])],
        rows: [
          {
            row: 'tested',
            indicator: { title: 'Tested', calculation: 'percentage' },
            current: {
              value: '50',
              count: 5,
              denominator: 10,
              disaggregations: [
                { name: 'hiv', bands: [] },
                { name: 'visit', bands: [] },
              ],
              crossed: [
                { at: [{ name: 'hiv', code: 'positive' }, { name: 'visit', code: 'first' }], value: '66.67', count: 4, denominator: 6 },
                { at: [{ name: 'hiv', code: 'positive' }, { name: 'visit', code: 'later' }], value: null, count: 0, denominator: 0 },
                { at: [{ name: 'hiv', code: 'negative' }, { name: 'visit', code: 'first' }], value: null, count: 0, denominator: 0 },
                { at: [{ name: 'hiv', code: 'negative' }, { name: 'visit', code: 'later' }], value: '25', count: 1, denominator: 4 },
              ],
            },
            previous: null,
            error: null,
          },
        ],
      },
    ],
  }

  test('shows the summed crossed parts the server sends, and says why when it sends none', () => {
    const [grid] = runGrids(crossedReport.report, crossedRun, texts)
    expect(grid?.grid.header).toEqual(['Indicator', 'positive · first', 'positive · later', 'negative · first', 'negative · later'])
    expect(grid?.grid.rows[0]?.cells).toEqual(['66.67%', '—', '—', '25%'])
    const section = crossedRun.sections[0]
    if (section?.kind !== 'table' || section.rows[0]?.current === null || section.rows[0]?.current === undefined) throw new Error('a table')
    const without: ReportRun = { ...crossedRun, sections: [{ ...section, rows: [{ ...section.rows[0], current: { ...section.rows[0].current, crossed: [] } }] }] }
    expect(runGrids(crossedReport.report, without, texts)[0]?.grid.rows[0]?.cells[0]).toBe('n/a (crossed)')
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
