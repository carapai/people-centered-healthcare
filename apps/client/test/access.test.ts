import { describe, expect, test } from 'vitest'

import { AccessError, grant, grantsQuery, provision, revocationsQuery, revoke, rolesQuery } from '../src/features/access/api'
import { fakeTransport } from './fake'

describe('access', () => {
  test('the lists are read from /access, under one key root', async () => {
    const transport = fakeTransport((request) => ({ body: request.path === '/access/roles' ? ['clinician'] : [] }))
    await expect(rolesQuery(transport).queryFn({})).resolves.toEqual(['clinician'])
    await grantsQuery(transport).queryFn({})
    await revocationsQuery(transport).queryFn({})
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /access/roles', 'GET /access/grants', 'GET /access/revocations'])
    expect([rolesQuery, grantsQuery, revocationsQuery].map((q) => q(transport).queryKey[0])).toEqual(['local-access', 'local-access', 'local-access'])
  })

  test('grants, revocations and accounts are posted as forms', async () => {
    const transport = fakeTransport(() => ({ status: 201, body: {} }))
    await grant(transport, 'nakato', 'clinician', 3)
    await revoke(transport, 'nakato')
    await provision(transport, { username: 'auma', password: 'harbour violet 3318', role: 'clinician', days: 7 })
    expect(transport.sent.map((r) => [r.method, r.path, r.form])).toEqual([
      ['POST', '/access/grants', { username: 'nakato', role: 'clinician', days: '3' }],
      ['POST', '/access/revocations', { username: 'nakato' }],
      ['POST', '/access/accounts', { username: 'auma', password: 'harbour violet 3318', role: 'clinician', days: '7' }],
    ])
  })

  test('a refusal carries the status, code and detail, so the page can explain it', async () => {
    const forbidden = fakeTransport(() => ({ status: 403, body: { error: 'forbidden', detail: 'offline administration needs access.administer-local' } }))
    await expect(rolesQuery(forbidden).queryFn({})).rejects.toEqual(new AccessError(403, 'forbidden', 'offline administration needs access.administer-local'))
    const taken = fakeTransport(() => ({ status: 409, body: { error: 'taken', detail: 'the username is taken' } }))
    await expect(provision(taken, { username: 'auma', password: 'x', role: 'clinician', days: 1 })).rejects.toMatchObject({ status: 409, code: 'taken' })
  })
})
