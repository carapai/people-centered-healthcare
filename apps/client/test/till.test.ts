import { describe, expect, test } from 'vitest'

import { TillError, closeShift, openShiftQuery, reconcile, takePayment } from '../src/features/till/api'
import { fakeTransport } from './fake'

describe('till', () => {
  test('a closed till has no shift, and an open one comes with its payments', async () => {
    const closed = fakeTransport(() => ({ status: 404 }))
    await expect(openShiftQuery(closed, 'till-1').queryFn()).resolves.toBeNull()
    expect(closed.sent[0]).toMatchObject({ method: 'GET', path: '/payments/shifts/open?till=till-1' })
    const open = fakeTransport((r) => (r.path.startsWith('/payments/shifts/open') ? { body: { id: 's1' } } : { body: { shift: { id: 's1' }, payments: [] } }))
    await expect(openShiftQuery(open, 'till-1').queryFn()).resolves.toMatchObject({ shift: { id: 's1' }, payments: [] })
    expect(open.sent[1]).toMatchObject({ path: '/payments/shifts/s1' })
  })

  test('cash is taken at the till; other methods are pending with their reference', async () => {
    const transport = fakeTransport(() => ({ status: 201, body: {} }))
    await takePayment(transport, 's1', 'cash', '30000', 'Account/a', '')
    await takePayment(transport, 's1', 'mtn-momo', '20000', 'Account/a', 'x-ref-1')
    expect(transport.sent.map((r) => [r.path, r.body])).toEqual([
      ['/payments/cash', { shift: 's1', amount: '30000', payer: 'Account/a' }],
      ['/payments/pending', { shift: 's1', method: 'mtn-momo', amount: '20000', payer: 'Account/a', reference: 'x-ref-1' }],
    ])
  })

  test('closing, statements, and refusals carrying their reason', async () => {
    const ok = fakeTransport(() => ({ body: { confirmed: ['p1'], mismatched: [], unknown: [] } }))
    await expect(reconcile(ok, 'x-ref-1', '20000')).resolves.toMatchObject({ confirmed: ['p1'] })
    expect(ok.sent[0]).toMatchObject({ path: '/payments/statements', body: { lines: [{ reference: 'x-ref-1', amount: '20000' }] } })
    await expect(closeShift(fakeTransport(() => ({ status: 403, body: { error: 'needs payments.cash' } })), 's1', '1')).rejects.toEqual(
      new TillError(403, 'needs payments.cash'),
    )
  })
})
