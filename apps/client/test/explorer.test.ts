import { describe, expect, test } from 'vitest'

import { type Answer, type Explorer, ExplorerError, type Question, answerQuery, chartSeries, explorerQuery, newestComputed, periodOf, recentPeriods } from '../src/features/explorer/api'
import { fakeTransport } from './fake'

const explorer: Explorer = {
  where: 'central',
  place: { code: 'district-kampala', name: 'Kampala', level: 'district', parent: 'region-central' },
  places: [
    { code: 'KAM013', name: 'Kisenyi HC IV', level: 'facility', parent: 'district-kampala' },
    { code: 'KAM031', name: 'Kitebi HC III', level: 'facility', parent: 'district-kampala' },
  ],
  indicators: [
    { measure: 'https://x/Measure/anc1|1.0.0', title: 'ANC 1st visits', calculation: 'count', per: null, rollsUp: true },
    { measure: 'https://x/Measure/ga|1.0.0', title: 'Gestational age at 1st visit', calculation: 'median', per: null, rollsUp: false },
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
