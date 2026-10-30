import { ResponseError } from '@health/api-client'
import { expect, test } from 'vitest'

import { operationClient } from '../src/platform/operations'
import { fakeTransport } from './fake'

test('a documented response is validated and typed', async () => {
  const transport = fakeTransport(() => ({ status: 503, body: { status: 'not-ready', checks: { database: 'fail' } } }))
  const result = await operationClient(transport).call('getReady')
  expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/ready' })
  expect(result).toEqual({ status: 503, body: { status: 'not-ready', checks: { database: 'fail' } } })
})

test('an undocumented status or a mismatched body is a ResponseError', async () => {
  await expect(operationClient(fakeTransport(() => ({ status: 500 }))).call('getHealth')).rejects.toBeInstanceOf(ResponseError)
  await expect(operationClient(fakeTransport(() => ({ body: { status: 'fail' } }))).call('getHealth')).rejects.toBeInstanceOf(ResponseError)
})
