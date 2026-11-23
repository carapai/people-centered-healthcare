import { describe, expect, test } from 'vitest'

import { DashboardsError, changeDashboard, createDashboard, dashboardsQuery, explorerSearch, readExplorerSearch, saveVisualization, sendable, splitList } from '../src/features/dashboards/api'
import { type Explorer, type Visualization, forViewer } from '../src/features/explorer/api'
import { fakeTransport } from './fake'

const kampala: Explorer = {
  where: 'central',
  place: { code: 'district-kampala', name: 'Kampala', level: 'district', parent: 'region-central' },
  places: [
    { code: 'KAM013', name: 'Kisenyi HC IV', level: 'facility', parent: 'district-kampala' },
    { code: 'KAM020', name: 'Kawempe HC III', level: 'facility', parent: 'district-kampala' },
  ],
  indicators: [],
  periodTypes: ['monthly'],
}

const wakiso: Explorer = {
  ...kampala,
  place: { code: 'district-wakiso', name: 'Wakiso', level: 'district', parent: 'region-central' },
  places: [{ code: 'WAK001', name: 'Wakiso HC IV', level: 'facility', parent: 'district-wakiso' }],
}

const saved: Visualization = {
  type: 'column',
  layout: 'placesByIndicators',
  series: 'place',
  follow: 'eachAreaUnder',
  question: { indicators: ['https://x/Measure/anc1|1.0.0'], periodType: 'monthly', from: '2026-01', to: '2026-03', places: ['KAM013', 'KAM020'] },
}

describe('dashboards', () => {
  test('a Visualization that follows the viewer shows each viewer their own area', () => {
    // Saved in Kampala over each area under it; Wakiso's user sees Wakiso's facilities.
    expect(forViewer(wakiso, saved).question.places).toEqual(['WAK001'])
    expect(forViewer(kampala, saved).question.places).toEqual(['KAM013', 'KAM020'])
    expect(forViewer(wakiso, { ...saved, follow: 'myPlace' }).question.places).toEqual(['district-wakiso'])
    // Places named by hand stay as saved: the server refuses those outside the viewer's area.
    const { follow: _none, ...named } = saved
    expect(forViewer(wakiso, named)).toBe(named)
  })

  test('Open in explorer carries the setup, and only a setup the explorer can draw opens', () => {
    const { v } = explorerSearch(saved)
    expect(readExplorerSearch(v)).toEqual(saved)
    expect(readExplorerSearch(undefined)).toBeUndefined()
    expect(readExplorerSearch('not json')).toBeUndefined()
    expect(readExplorerSearch(JSON.stringify({ ...saved, type: 'pie' }))).toBeUndefined()
    expect(readExplorerSearch(JSON.stringify({ ...saved, follow: 'everyone' }))).toBeUndefined()
    expect(readExplorerSearch(JSON.stringify({ ...saved, question: { ...saved.question, places: 'KAM013' } }))).toBeUndefined()
  })

  test('the API lists, saves and shares', async () => {
    const listed = { where: 'central', canAuthor: true, dashboards: [{ id: 'd1', title: 'ANC', kind: 'saved', mine: true, shares: [], items: [] }], visualizations: [] }
    const transport = fakeTransport((request) => (request.method === 'GET' ? { body: listed } : request.method === 'POST' ? { status: 201, body: { id: 'x', title: 'ANC' } } : { body: { id: 'd1' } }))
    expect(await dashboardsQuery(transport).queryFn({})).toEqual({ canAuthor: true, dashboards: listed.dashboards, visualizations: [] })
    await createDashboard(transport, { title: 'ANC', shares: [{ with: 'level', level: 'district' }] })
    await saveVisualization(transport, { title: 'Visits', visualization: saved, dashboard: 'd1' })
    await changeDashboard(transport, 'd1', { title: 'ANC', shares: [sendable({ with: 'person', username: 'e2e-wakiso', account: 'a-2' })] })
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /analytics/dashboards', 'POST /analytics/dashboards', 'POST /analytics/visualizations', 'PUT /analytics/dashboards/d1'])
    expect(transport.sent[2]?.body).toEqual({ title: 'Visits', visualization: saved, dashboard: 'd1' })
    expect(transport.sent[3]?.body).toEqual({ title: 'ANC', shares: [{ with: 'person', username: 'e2e-wakiso' }] })

    const refused = fakeTransport(() => ({ status: 403, body: { detail: 'saving Visualizations and Dashboards needs analytics.author' } }))
    await expect(saveVisualization(refused, { title: 'Visits', visualization: saved })).rejects.toEqual(new DashboardsError(403, 'saving Visualizations and Dashboards needs analytics.author'))
  })

  test('roles and people are a comma-separated list, each once', () => {
    expect(splitList(' district, in-charge,,district ')).toEqual(['district', 'in-charge'])
    expect(splitList('')).toEqual([])
  })
})
