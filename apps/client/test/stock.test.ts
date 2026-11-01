import { describe, expect, test } from 'vitest'

import { StockError, lineComplete, onHandQuery, readGs1, receiveGoods, reorderQuery } from '../src/features/stock/api'
import { fakeTransport } from './fake'

const line = { product: 'amox', batch: 'B42', expiry: '2027-06-30', quantity: 1000, pack_size: 100, unit_cost: '1.50' }

describe('stock', () => {
  test('a goods received note is sent and refusals carry their reason', async () => {
    const transport = fakeTransport(() => ({ status: 201, body: { id: 'grn-1' } }))
    const note = { store: 'Location/pharmacy', supplier: 'NMS', delivery: 'INV-42', lines: [line] }
    await expect(receiveGoods(transport, note)).resolves.toBe('grn-1')
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/stock/goods-received', body: note })
    await expect(receiveGoods(fakeTransport(() => ({ status: 422, body: { error: 'line 0 has a negative cost' } })), note)).rejects.toEqual(
      new StockError(422, 'line 0 has a negative cost'),
    )
  })

  test('a line is complete with its batch, expiry, quantities and a decimal cost', () => {
    expect(lineComplete(line)).toBe(true)
    expect(lineComplete({ ...line, expiry: '06/2027' })).toBe(false)
    expect(lineComplete({ ...line, quantity: 0 })).toBe(false)
    expect(lineComplete({ ...line, unit_cost: 'free' })).toBe(false)
    expect(lineComplete({ ...line, batch: ' ' })).toBe(false)
  })

  test('stock on hand, reorder and barcodes', async () => {
    const transport = fakeTransport(() => ({ body: [] }))
    await onHandQuery(transport, 'Location/pharmacy').queryFn({})
    await reorderQuery(transport, 'Location/pharmacy').queryFn({})
    expect(transport.sent.map((r) => r.path)).toEqual(['/stock/on-hand?store=Location%2Fpharmacy', '/stock/reorder?store=Location%2Fpharmacy&days=30'])
    const scanned = fakeTransport(() => ({ body: { gtin: '09501101530003', expiry: '2027-06-30', batch: 'B42', serial: null } }))
    await expect(readGs1(scanned, '(01)09501101530003(17)270630(10)B42')).resolves.toMatchObject({ batch: 'B42' })
  })
})
