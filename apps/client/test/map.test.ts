import { describe, expect, test } from 'vitest'

import { readExplorerSearch } from '../src/features/dashboards/api'
import type { Answer, Explorer, Question, ThresholdBand } from '../src/features/explorer/api'
import { type MapPlaces, drawnMap, mapQuery } from '../src/features/explorer/map'
import { fakeTransport } from './fake'

// #604: a map Visualization over the national core's boundaries and the NHFR's coordinates.

const POSITIVITY = 'https://x/Measure/malaria-positivity|1.0.0'
const bands: ThresholdBand[] = [
  { from: null, to: '20', tone: 'good' },
  { from: '20', to: '40', tone: 'warning' },
  { from: '40', to: null, tone: 'bad' },
]

/** A national user: the regions, a district in each, and their facilities. */
const explorer: Explorer = {
  where: 'central',
  place: { code: 'central', name: 'Ministry of Health', level: 'national', parent: null },
  places: [
    { code: 'region-central', name: 'Central', level: 'region', parent: 'central' },
    { code: 'region-northern', name: 'Northern', level: 'region', parent: 'central' },
    { code: 'region-western', name: 'Western', level: 'region', parent: 'central' },
    { code: 'district-kampala', name: 'Kampala', level: 'district', parent: 'region-central' },
    { code: 'district-gulu', name: 'Gulu', level: 'district', parent: 'region-northern' },
    { code: 'KAM013', name: 'Kisenyi HC IV', level: 'facility', parent: 'district-kampala' },
    { code: 'KAM031', name: 'Kitebi HC III', level: 'facility', parent: 'district-kampala' },
    { code: 'GUL001', name: 'Gulu RRH', level: 'facility', parent: 'district-gulu' },
  ],
  indicators: [{ measure: POSITIVITY, title: 'Malaria test positivity', calculation: 'percentage', per: null, rollsUp: true, thresholds: bands, disaggregations: [] }],
  periodTypes: ['quarterly'],
}

const square = (x: number, y: number, size: number): [number, number][][] => [
  [
    [x, y],
    [x + size, y],
    [x + size, y + size],
    [x, y + size],
    [x, y],
  ],
]

const places: MapPlaces = {
  areas: [
    { code: 'region-central', name: 'Central', level: 'region', geometry: { type: 'Polygon', coordinates: square(31, -1, 2.5) } },
    { code: 'region-northern', name: 'Northern', level: 'region', geometry: { type: 'Polygon', coordinates: square(31, 1.6, 2.5) } },
    { code: 'region-western', name: 'Western', level: 'region', geometry: { type: 'MultiPolygon', coordinates: [square(29.6, -1.4, 1.4)] } },
    { code: 'district-kampala', name: 'Kampala', level: 'district', geometry: { type: 'Polygon', coordinates: square(32.5, 0.25, 0.15) } },
    { code: 'district-gulu', name: 'Gulu', level: 'district', geometry: { type: 'Polygon', coordinates: square(32, 2.6, 0.5) } },
  ],
  facilities: [
    { code: 'KAM013', name: 'Kisenyi HC IV', parent: 'district-kampala', latitude: 0.3136, longitude: 32.5664 },
    { code: 'KAM031', name: 'Kitebi HC III', parent: 'district-kampala', latitude: 0.287, longitude: 32.559 },
    { code: 'GUL001', name: 'Gulu RRH', parent: 'district-gulu', latitude: 2.776, longitude: 32.299 },
  ],
}

const regions: Question = { indicators: [POSITIVITY], periodType: 'quarterly', from: '2026Q3', to: '2026Q4', places: ['region-central', 'region-northern', 'region-western'] }

const cell = (place: string, period: string, value: string | null, missing: Answer['cells'][number]['missing'] = []) => ({ indicator: POSITIVITY, period, place, value, missing })

const answer: Answer = {
  where: 'central',
  periods: [
    { id: '2026Q3', start: '2026-07-01', end: '2026-09-30' },
    { id: '2026Q4', start: '2026-10-01', end: '2026-12-31' },
  ],
  cells: [
    cell('region-central', '2026Q3', '10'),
    cell('region-central', '2026Q4', '45', [{ facility: 'KAM031', name: 'Kitebi HC III', lastSync: null }]),
    cell('region-northern', '2026Q4', '25'),
    cell('region-western', '2026Q4', null),
    cell('KAM013', '2026Q4', '45'),
  ],
}

describe('maps', () => {
  test("a national user's malaria map shades each region by the band of its last period", () => {
    const map = drawnMap(answer, explorer, regions, places, POSITIVITY)
    expect(map?.period).toBe('2026Q4')
    expect(map?.areas.map((a) => [a.code, a.picked, a.value, a.tone])).toEqual([
      ['region-central', true, 45, 'bad'],
      ['region-northern', true, 25, 'warning'],
      ['region-western', true, null, null],
    ])
    // A path per ring, inside the drawing.
    for (const area of map?.areas ?? []) expect(area.path).toMatch(/^M[\d.]+,[\d.]+(L[\d.]+,[\d.]+)+Z/)
    const coordinates = (map?.areas ?? []).flatMap((a) => [...a.path.matchAll(/([\d.]+),([\d.]+)/g)].map((m) => [Number(m[1]), Number(m[2])]))
    expect(Math.max(...coordinates.map(([x]) => x ?? 0))).toBeLessThanOrEqual(map?.width ?? 0)
    expect(Math.max(...coordinates.map(([, y]) => y ?? 0))).toBeLessThanOrEqual(map?.height ?? 0)
    // Every facility inside a region asked, uncoloured: they were not asked themselves.
    expect(map?.points.map((p) => [p.code, p.picked, p.tone])).toEqual([
      ['KAM013', false, null],
      ['KAM031', false, null],
      ['GUL001', false, null],
    ])
  })

  test('an unsynced facility shows hollow, with its last sync', () => {
    const map = drawnMap(answer, explorer, regions, places, POSITIVITY)
    const kitebi = map?.points.find((p) => p.code === 'KAM031')
    expect(kitebi?.missing).toEqual({ facility: 'KAM031', name: 'Kitebi HC III', lastSync: null })
    expect(map?.points.find((p) => p.code === 'KAM013')?.missing).toBeNull()
  })

  test('a district map draws the district asked over its region, and colours a facility asked by its own figure', () => {
    const question: Question = { ...regions, places: ['district-kampala', 'KAM013'] }
    const map = drawnMap(answer, explorer, question, places, POSITIVITY)
    // The region around it is the backdrop, first and uncoloured; the district has no figure here.
    expect(map?.areas.map((a) => [a.code, a.picked])).toEqual([
      ['region-central', false],
      ['district-kampala', true],
    ])
    expect(map?.points.map((p) => [p.code, p.picked, p.value, p.tone])).toEqual([
      ['KAM013', true, 45, 'bad'],
      ['KAM031', false, null, null],
    ])
  })

  test('nothing to draw gives no map', () => {
    expect(drawnMap(answer, explorer, regions, { areas: [], facilities: [] }, POSITIVITY)).toBeNull()
  })

  test('the map is read from the API, its coordinates as plain numbers', async () => {
    const transport = fakeTransport(() => ({ body: places }))
    expect(await mapQuery(transport).queryFn({})).toEqual(places)
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /analytics/map'])
  })

  test('a saved map opens in the explorer', () => {
    const shown = { type: 'map', question: regions, layout: 'placesByIndicators', series: 'indicator', follow: 'eachAreaUnder' }
    expect(readExplorerSearch(JSON.stringify(shown))?.type).toBe('map')
  })
})
