import { describe, expect, test } from 'vitest'

import { ConflictError, candidateFor, conflictQuery, conflictsKey, conflictsQuery, content, resolve, type ConflictDetail } from '../src/features/conflicts/api'
import { fakeTransport } from './fake'

const summary = {
  id: 'tx-1',
  peer: 'edge-1',
  class: 'identity',
  policy: 'Conflict Record resolved at central by registry staff. Never auto-merged.',
  status: 'open',
  openedAt: '2026-11-10T08:00:00Z',
  overdue: false,
  items: [{ op_id: 'op-1', resource: 'Patient/p1', parent: 'v1', head: 'v2', class: 'identity' }],
}

describe('conflict review', () => {
  test('the queue lists the open records, and a refusal carries its status', async () => {
    const ok = fakeTransport(() => ({ body: { conflicts: [summary] } }))
    const query = conflictsQuery(ok)
    expect(query.queryKey).toEqual(conflictsKey)
    await expect(query.queryFn()).resolves.toEqual([summary])
    expect(ok.sent[0]).toMatchObject({ method: 'GET', path: '/sync/conflicts' })
    const refused = fakeTransport(() => ({ status: 403, body: { detail: 'sync.resolve-conflicts is required' } }))
    await expect(conflictsQuery(refused).queryFn()).rejects.toEqual(new ConflictError(403, 'sync.resolve-conflicts is required'))
  })

  test('one record is read with both branches, and the candidate is found by its resource', async () => {
    const detail: ConflictDetail = {
      ...summary,
      candidates: [{ resource_type: 'Patient', resource_id: 'p1', op_id: 'op-1', body: { resourceType: 'Patient', meta: { versionId: 'op-1' }, gender: 'female' } }],
      heads: { 'Patient/p1': { resourceType: 'Patient', meta: { versionId: 'v2' }, gender: 'male' } },
    }
    const transport = fakeTransport(() => ({ body: detail }))
    const read = await conflictQuery(transport, 'tx-1').queryFn()
    expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/sync/conflicts/tx-1' })
    expect(candidateFor(read, 'Patient/p1')?.op_id).toBe('op-1')
    expect(candidateFor(read, 'Patient/p2')).toBeUndefined()
    // What a reviewer compares and writes leaves out each node's `meta`.
    expect(content(read.heads['Patient/p1'])).toEqual({ resourceType: 'Patient', gender: 'male' })
    expect(content(null)).toBeUndefined()
  })

  test('a decision is posted to its record, and a refused one carries the reason', async () => {
    const transport = fakeTransport(() => ({ body: { id: 'tx-1', status: 'resolved' } }))
    await resolve(transport, 'tx-1', { decision: 'keep-head', resource: 'Patient/p1' })
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/sync/conflicts/tx-1', body: { decision: 'keep-head', resource: 'Patient/p1' } })
    await resolve(transport, 'tx-1', { decision: 'write', resource: 'Patient/p1', json: { resourceType: 'Patient' } })
    expect(transport.sent[1]?.body).toEqual({ decision: 'write', resource: 'Patient/p1', json: { resourceType: 'Patient' } })
    const refused = fakeTransport(() => ({ status: 422, body: { detail: 'the decision is about another record' } }))
    await expect(resolve(refused, 'tx-1', { decision: 'keep-head', resource: 'Patient/p9' })).rejects.toEqual(new ConflictError(422, 'the decision is about another record'))
  })
})
