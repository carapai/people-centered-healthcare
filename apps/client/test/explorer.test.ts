import { describe, expect, test } from 'vitest'

import { type Answer, type Explorer, ExplorerError, type ExplorerIndicator, type Question, type ThresholdBand, answerQuery, chartSeries, explorerQuery, newestComputed, periodOf, previousPeriod, recentPeriods, singleNumbers, tableOf, toneOf } from '../src/features/explorer/api'
import { fakeTransport } from './fake'

const explorer: Explorer = {
  where: 'central',
  place: { code: 'district-kampala', name: 'Kampala', level: 'district', parent: 'region-central' },
  places: [
    { code: 'KAM013', name: 'Kisenyi HC IV', level: 'facility', parent: 'district-kampala' },
    { code: 'KAM031', name: 'Kitebi HC III', level: 'facility', parent: 'district-kampala' },
  ],
  indicators: [
    { measure: 'https://x/Measure/anc1|1.0.0', title: 'ANC 1st visits', calculation: 'count', per: null, rollsUp: true, thresholds: [] },
    { measure: 'https://x/Measure/ga|1.0.0', title: 'Gestational age at 1st visit', calculation: 'median', per: null, rollsUp: false, thresholds: [] },
  ],
  periodTypes: ['weekly', 'monthly', 'quarterly', 'financialJuly'],
}

const question: Question = { indicators: ['https://x/Measure/anc1|1.0.0'], periodType: 'monthly', from: '2026-02', to: '2026-03', places: ['district-kampala', 'KAM013'] }

const answer: Answer = {
  where: 'central',
  periods: [
    { id: '2026-02', start: '2026-02-01', end: '2026-02-28' },
    { id: '2026-03', start: '2026-03-01', end: '2026-03-31' },
  ],
  cells: [
    { indicator: question.indicators[0] ?? '', period: '2026-02', place: 'district-kampala', value: null, reported: 0, expected: 3, missing: [], computedAt: null },
    { indicator: question.indicators[0] ?? '', period: '2026-03', place: 'district-kampala', value: '10', reported: 2, expected: 3, missing: [{ facility: 'KAM031', name: 'Kitebi HC III', lastSync: null }], computedAt: '2026-04-01T06:00:00Z' },
    { indicator: question.indicators[0] ?? '', period: '2026-02', place: 'KAM013', value: '3', reported: 1, expected: 1, missing: [], computedAt: '2026-03-01T06:00:00Z' },
    { indicator: question.indicators[0] ?? '', period: '2026-03', place: 'KAM013', value: '4', reported: 1, expected: 1, missing: [], computedAt: '2026-04-02T06:00:00Z' },
  ],
}

describe('explorer', () => {
  test('periods are written as the server writes standard periods', () => {
    const day = new Date(Date.UTC(2026, 2, 31))
    expect(periodOf('monthly', day)).toBe('2026-03')
    expect(periodOf('quarterly', day)).toBe('2026Q1')
    expect(periodOf('financialJuly', day)).toBe('2025July')
    expect(periodOf('financialJuly', new Date(Date.UTC(2026, 6, 1)))).toBe('2026July')
    expect(periodOf('weekly', day)).toBe('2026W14')
    expect(periodOf('weekly', new Date(Date.UTC(2027, 0, 1)))).toBe('2026W53')
    expect(periodOf('weekly', new Date(Date.UTC(2025, 11, 29)))).toBe('2026W01')
  })

  test('the range pickers offer whole periods only, oldest first', () => {
    const day = new Date(Date.UTC(2026, 2, 31))
    expect(recentPeriods('monthly', day, 4)).toEqual(['2025-12', '2026-01', '2026-02', '2026-03'])
    expect(recentPeriods('quarterly', day, 3)).toEqual(['2025Q3', '2025Q4', '2026Q1'])
    expect(recentPeriods('weekly', day, 2)).toEqual(['2026W13', '2026W14'])
    expect(recentPeriods('financialJuly', day, 2)).toEqual(['2024July', '2025July'])
  })

  test('the explorer and its answers are read from the API', async () => {
    const transport = fakeTransport((request) => ({ body: request.method === 'GET' ? explorer : answer }))
    expect(await explorerQuery(transport).queryFn({})).toEqual(explorer)
    expect(await answerQuery(transport, question).queryFn({})).toEqual(answer)
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /analytics/explorer', 'POST /analytics/explore'])
    expect(transport.sent[1]?.body).toEqual(question)
    const refused = fakeTransport(() => ({ status: 403, body: { detail: 'district-wakiso is not in your area' } }))
    await expect(answerQuery(refused, question).queryFn({})).rejects.toEqual(new ExplorerError(403, 'district-wakiso is not in your area'))
  })

  test('the chart has a series per indicator and place, with no figure where none reported', () => {
    const series = chartSeries(answer, explorer, question)
    expect(series.map((s) => [s.label, s.values])).toEqual([
      ['ANC 1st visits, Kampala', [null, 10]],
      ['ANC 1st visits, Kisenyi HC IV', [3, 4]],
    ])
    expect(newestComputed(answer)).toBe('2026-04-02T06:00:00Z')
  })
})

// #600: line, table and single-number Visualizations, with threshold colours.
describe('visualizations', () => {
  const positivity = 'https://x/Measure/malaria-positivity|1.0.0'
  const bands: ThresholdBand[] = [
    { from: null, to: '20', tone: 'good' },
    { from: '20', to: '40', tone: 'warning' },
    { from: '40', to: null, tone: 'bad' },
  ]
  const withPositivity: Explorer = { ...explorer, indicators: [...explorer.indicators, { measure: positivity, title: 'Malaria test positivity', calculation: 'percentage', per: null, rollsUp: true, thresholds: bands }] }

  test('a malaria positivity of 45% is in the bad band, 39.99% in the warning one, and an indicator without bands has no tone', () => {
    expect(toneOf(bands, 45)).toBe('bad')
    expect(toneOf(bands, 40)).toBe('bad')
    expect(toneOf(bands, 39.99)).toBe('warning')
    expect(toneOf(bands, 3)).toBe('good')
    expect(toneOf(bands, null)).toBeNull()
    expect(toneOf([], 45)).toBeNull()
    expect(toneOf([{ from: null, to: '50', tone: 'bad' }], 60)).toBeNull()
  })

  test('the indicators read from the API carry their bands, none when the server sends none', async () => {
    const { thresholds: _bands, ...bare } = withPositivity.indicators[0] as ExplorerIndicator
    const transport = fakeTransport(() => ({ body: { ...withPositivity, indicators: [bare, withPositivity.indicators[2]] } }))
    const read = await explorerQuery(transport).queryFn({})
    expect(read.indicators.map((i) => i.thresholds)).toEqual([[], bands])
  })

  test('the period before is of the same type', () => {
    expect(previousPeriod('quarterly', '2026Q1')).toBe('2025Q4')
    expect(previousPeriod('quarterly', '2026Q3')).toBe('2026Q2')
    expect(previousPeriod('monthly', '2026-01')).toBe('2025-12')
    expect(previousPeriod('monthly', '2026-10')).toBe('2026-09')
    expect(previousPeriod('weekly', '2026W01')).toBe('2025W52')
    expect(previousPeriod('weekly', '2027W01')).toBe('2026W53')
    expect(previousPeriod('weekly', '2026W14')).toBe('2026W13')
    expect(previousPeriod('financialJuly', '2026July')).toBe('2025July')
  })

  test('a single number is the last period’s figure with its change on the period before', () => {
    const q: Question = { indicators: [positivity], periodType: 'quarterly', from: '2026Q2', to: '2026Q2', places: ['district-kampala'] }
    const cell = (period: string, value: string | null) => ({ indicator: positivity, period, place: 'district-kampala', value, reported: 2, expected: 3, missing: [], computedAt: null })
    const now: Answer = { where: 'central', periods: [{ id: '2026Q2', start: '2026-04-01', end: '2026-06-30' }], cells: [cell('2026Q2', '45')] }
    const before: Answer = { where: 'central', periods: [{ id: '2026Q1', start: '2026-01-01', end: '2026-03-31' }], cells: [cell('2026Q1', '30')] }
    expect(singleNumbers(now, before, withPositivity, q)).toEqual([{ indicator: positivity, place: 'district-kampala', label: 'Malaria test positivity', period: '2026Q2', value: 45, previousPeriod: '2026Q1', previous: 30, change: 15, tone: 'bad' }])
    // Without a figure the period before, there is no change to show.
    expect(singleNumbers(now, { ...before, cells: [cell('2026Q1', null)] }, withPositivity, q)[0]).toMatchObject({ previous: null, change: null })
    // The period before may already be in the range asked.
    const both: Answer = { ...now, periods: [...before.periods, ...now.periods], cells: [cell('2026Q1', '30'), cell('2026Q2', '45')] }
    expect(singleNumbers(both, undefined, withPositivity, { ...q, from: '2026Q1' })[0]).toMatchObject({ previous: 30, change: 15 })
  })

  test('a table lays out places by indicators or indicators by periods, each figure with its tone', () => {
    const q: Question = { indicators: [question.indicators[0] ?? '', positivity], periodType: 'monthly', from: '2026-02', to: '2026-03', places: ['district-kampala', 'KAM013'] }
    const cells = [...answer.cells, ...['district-kampala', 'KAM013'].flatMap((place) => ['2026-02', '2026-03'].map((period) => ({ indicator: positivity, period, place, value: place === 'KAM013' ? '45' : '12.5', computedAt: null })))]
    const a: Answer = { ...answer, cells }
    const byPlace = tableOf(a, withPositivity, q, 'placesByIndicators')
    expect(byPlace.columns.map((c) => c.label)).toEqual(['ANC 1st visits, 2026-02', 'ANC 1st visits, 2026-03', 'Malaria test positivity, 2026-02', 'Malaria test positivity, 2026-03'])
    expect(byPlace.rows.map((r) => [r.label, r.cells.map((c) => [c.value, c.tone])])).toEqual([
      ['Kampala', [[null, null], [10, null], [12.5, 'good'], [12.5, 'good']]],
      ['Kisenyi HC IV', [[3, null], [4, null], [45, 'bad'], [45, 'bad']]],
    ])
    const byPeriod = tableOf(a, withPositivity, q, 'indicatorsByPeriods')
    expect(byPeriod.columns.map((c) => c.label)).toEqual(['2026-02', '2026-03'])
    expect(byPeriod.rows.map((r) => r.label)).toEqual(['ANC 1st visits, Kampala', 'ANC 1st visits, Kisenyi HC IV', 'Malaria test positivity, Kampala', 'Malaria test positivity, Kisenyi HC IV'])
    expect(byPeriod.rows[3]?.cells.map((c) => c.tone)).toEqual(['bad', 'bad'])
    // With one period, its id is left out of the column labels.
    const one = tableOf({ ...a, periods: a.periods.slice(1) }, withPositivity, { ...q, from: '2026-03' }, 'placesByIndicators')
    expect(one.columns.map((c) => c.label)).toEqual(['ANC 1st visits', 'Malaria test positivity'])
  })

  test('the chart series carry their indicator’s bands', () => {
    const q: Question = { ...question, indicators: [positivity] }
    expect(chartSeries(answer, withPositivity, q).map((s) => s.thresholds)).toEqual([bands, bands])
  })
})
