import { describe, expect, test } from 'vitest'

import { BreakGlassError, acknowledge, alertsQuery, decide, reviewsQuery } from '../src/features/break-glass/api'
import { worklistKey } from '../src/platform'
import { fakeTransport } from './fake'

const grant = {
  id: 'g1',
  account: 'u1',
  facility: 'KAM013',
  patient: 'Patient/p1',
  reason: 'BTG',
  justification: 'unconscious on arrival',
  at: '2026-10-31T08:00:00Z',
  expires_at: '2026-10-31T09:00:00Z',
  review_due: '2026-11-03T08:00:00Z',
  acknowledged: false,
}

describe('break-glass', () => {
  test('the lists are keyed by the worklists the server hints, so a grant refreshes them', async () => {
    const transport = fakeTransport(() => ({ body: [grant] }))
    const alerts = alertsQuery(transport)
    expect(alerts.queryKey).toEqual(worklistKey('break-glass-alerts'))
    await expect(alerts.queryFn({})).resolves.toEqual([grant])
    const reviews = reviewsQuery(transport)
    expect(reviews.queryKey).toEqual(worklistKey('break-glass-reviews'))
    await reviews.queryFn({})
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /break-glass/alerts', 'GET /break-glass/reviews'])
  })

  test('a refused list carries its status, so the page tells a role from a failure', async () => {
    const transport = fakeTransport(() => ({ status: 403, body: { resourceType: 'OperationOutcome' } }))
    await expect(alertsQuery(transport).queryFn({})).rejects.toEqual(new BreakGlassError(403))
  })

  test('acknowledging and deciding post to the grant', async () => {
    const transport = fakeTransport(() => ({ status: 204 }))
    await acknowledge(transport, 'g/1')
    await decide(transport, 'g1', false, 'not this patient')
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/break-glass/alerts/g%2F1/acknowledge' })
    expect(transport.sent[1]).toMatchObject({
      method: 'POST',
      path: '/break-glass/reviews/g1',
      body: { decision: 'inappropriate', note: 'not this patient' },
    })
    const refused = fakeTransport(() => ({ status: 400 }))
    await expect(decide(refused, 'g1', false, '')).rejects.toEqual(new BreakGlassError(400))
  })
})
