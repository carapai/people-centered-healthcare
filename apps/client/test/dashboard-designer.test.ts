import type { StudioDashboardPreview } from '@health/api-client'
import { createIntl } from 'react-intl'
import { describe, expect, test } from 'vitest'

import overview from '../../../packages/reference-packages/hmis/configuration/dashboards/anc-overview.json'
import { chartSeries, singleNumbers, tableOf } from '../src/features/explorer/api'
import {
  addItem,
  moveItem,
  newDashboard,
  previewAnswer,
  previewExplorer,
  previewShown,
  readDashboard,
  removeItem,
  toggleIndicator,
  updateItem,
  withDisaggregation,
  withItem,
  withPeriodType,
  withPeriods,
  withSettings,
  writeDashboard,
  type Dashboard,
} from '../src/features/studio/dashboards/dashboard'
import { newDesign } from '../src/features/studio/designs'
import { dashboardDesignerMessages as bm } from '../src/i18n/messages'

// The Dashboard designer (#879): a standard Dashboard built step by step as the designer builds
// it is the same definition as the reference package's, read and written back unchanged, and its
// preview is drawn with the explorer's own series, tables and single numbers.

const FIRST = 'https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0'
const EARLY = 'https://health.go.ug/fhir/Measure/anc-first-visit-before-12-weeks|1.0.0'
const BANDS = 'https://health.go.ug/fhir/Library/age-bands-anc|1.0.0'

const read = (value: unknown): Dashboard => {
  const out = readDashboard(JSON.stringify(value))
  if ('refusal' in out) throw new Error(out.refusal)
  return out.dashboard
}

describe('reading and writing', () => {
  test('the reference Dashboard reads and writes back unchanged', () => {
    expect(writeDashboard(read(overview))).toEqual(overview)
  })

  test('a file that is not a Dashboard is refused with the reason', () => {
    expect(readDashboard('{')).toEqual({ refusal: 'not JSON' })
    expect(readDashboard('{"id": "x"}')).toEqual({ refusal: 'not written to the dashboard/1 schema' })
    expect(readDashboard(JSON.stringify({ ...overview, items: [{ id: 'a' }] }))).toEqual({ refusal: 'an item has no visualization' })
  })

  test('a fixed range is kept as written', () => {
    const fixed = structuredClone(overview) as Record<string, unknown> & typeof overview
    const question = fixed.items[0]!.visualization.question as Record<string, unknown>
    delete question['last']
    question['from'] = '2026-01'
    question['to'] = '2026-03'
    expect(writeDashboard(read(fixed))).toEqual(fixed)
  })
})

test('the designer builds the reference Dashboard step by step', () => {
  let d = withSettings(newDashboard('anc-overview', 'ANC overview'), { description: overview.description })
  d = addItem(d, 'ANC 1st visits by area', FIRST)
  d = updateItem(d, 0, (i) => withItem(i, { id: 'first-visits-by-area', series: 'place', follow: 'eachAreaUnder' }))
  d = addItem(d, 'ANC 1st visits by age band', FIRST)
  d = updateItem(d, 1, (i) => withItem(withDisaggregation(i, BANDS), { id: 'first-visits-by-age', series: 'band' }))
  d = addItem(d, 'ANC 1st visit before 12 weeks')
  d = updateItem(d, 2, (i) => withItem(withPeriods(toggleIndicator(i, EARLY, true), { last: 12 }), { id: 'before-12-weeks', type: 'line', layout: 'indicatorsByPeriods' }))
  d = addItem(d, 'ANC 1st visits this quarter', FIRST)
  d = updateItem(d, 3, (i) => withItem(withPeriods(withPeriodType(i, 'quarterly'), { last: 1 }), { id: 'first-visits-this-quarter', type: 'single' }))
  expect(writeDashboard(d)).toEqual(overview)
})

describe('editing', () => {
  const d = read(overview)

  test('items are added with an id of their own, moved and removed', () => {
    const added = addItem(addItem(newDashboard('x', 'X'), 'Visits'), 'Visits')
    expect(added.items.map((i) => i.id)).toEqual(['visits', 'visits-2'])
    expect(moveItem(d, 0, 1).items.map((i) => i.id).slice(0, 2)).toEqual(['first-visits-by-age', 'first-visits-by-area'])
    expect(moveItem(d, 0, -1)).toBe(d)
    expect(removeItem(d, 0).items).toHaveLength(3)
  })

  test('dropping the disaggregation drops band series; an emptied description is left out', () => {
    const banded = d.items[1]!
    expect(withDisaggregation(banded, '')).toMatchObject({ series: 'indicator', question: { indicators: [FIRST], periodType: 'monthly', periods: { last: 6 } } })
    expect(withDisaggregation(banded, '').question).not.toHaveProperty('disaggregation')
    expect(writeDashboard(withSettings(d, { description: ' ' }))).not.toHaveProperty('description')
  })

  test('indicators toggle on and off once each', () => {
    const item = d.items[0]!
    expect(toggleIndicator(item, FIRST, true)).toBe(item)
    expect(toggleIndicator(toggleIndicator(item, EARLY, true), FIRST, false).question.indicators).toEqual([EARLY])
  })
})

test('New dashboard creates one under configuration/dashboards/, its id its file name', () => {
  const made = newDesign('dashboard', 'ANC overview', 'https://health.go.ug/fhir', '1.1.0-draft', ['configuration/dashboards/anc-overview.json'])
  expect(made.path).toBe('configuration/dashboards/anc-overview-2.json')
  expect(made.resource).toEqual({ $schema: overview.$schema, id: 'anc-overview-2', title: 'ANC overview', items: [] })
})

describe('the preview', () => {
  const preview: StudioDashboardPreview = {
    asOf: '2026-04-30',
    problems: [],
    indicators: [{ measure: FIRST, title: 'ANC 1st visits', calculation: 'count', per: null, rollsUp: true, thresholds: [], disaggregations: [{ name: BANDS, by: 'age' }], package: 'ug.moh.anc@1.0.0', published: false }],
    items: [],
    sources: ['ug.moh.hmis@1.0.0', 'ug.moh.anc@1.0.0'],
    patients: 11,
    errors: [],
  }
  const explorer = previewExplorer(preview, 'Test patients')
  const answer = previewAnswer({
    periods: [
      { id: '2026-03', start: '2026-03-01', end: '2026-03-31' },
      { id: '2026-04', start: '2026-04-01', end: '2026-04-30' },
    ],
    cells: [
      { indicator: FIRST, period: '2026-03', place: 'sample', value: '4', bands: [{ code: '15-19', value: '1' }, { code: 'not-recorded', value: '3' }] },
      { indicator: FIRST, period: '2026-04', place: 'sample', value: '2', bands: [{ code: '15-19', value: '2' }, { code: 'not-recorded', value: null }] },
    ],
  })

  test('is drawn as the explorer draws, at the one sample place whatever the item follows', () => {
    const d = read(overview)
    const byArea = previewShown(d.items[0]!, '2026-03', '2026-04')
    expect(byArea.question.places).toEqual(['sample'])
    expect(byArea).not.toHaveProperty('follow')
    expect(chartSeries(answer, explorer, byArea.question, byArea.series).map((s) => [s.label, s.values])).toEqual([['Test patients', [4, 2]]])
    const byAge = previewShown(d.items[1]!, '2026-03', '2026-04')
    expect(chartSeries(answer, explorer, byAge.question, 'band').map((s) => s.values)).toEqual([
      [1, 2],
      [3, null],
    ])
    expect(tableOf(answer, explorer, byArea.question, 'placesByIndicators').rows).toHaveLength(1)
    const single = previewShown(d.items[3]!, '2026-04', '2026-04')
    expect(singleNumbers(answer, undefined, explorer, single.question)[0]).toMatchObject({ value: 2 })
  })

  test('a map is previewed as a table', () => {
    const d = read(overview)
    expect(previewShown({ ...d.items[0]!, type: 'map' }, '2026-03', '2026-04').type).toBe('table')
  })
})

test('every designer message formats with no ICU error', () => {
  const errors: string[] = []
  const intl = createIntl({ locale: 'en', onError: (e) => void errors.push(String(e)) })
  const values = { reason: 'r', path: 'p', id: 'i', type: 'column', indicators: 2, periods: 'last 6 monthly', count: 2, title: 't', patients: 11, sources: 's', from: 'a', to: 'b' }
  const texts = Object.values(bm).map((message) => intl.formatMessage(message, values))
  expect(errors).toEqual([])
  expect(texts).toContain('Column chart · 2 indicators · last 6 monthly')
})
