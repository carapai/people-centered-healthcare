import { describe, expect, test } from 'vitest'

import { type AuthStrategy, bearerAuth, dpopAuth } from '../src/platform/auth'
import { httpTransport } from '../src/platform/http'
import { memorySession } from '../src/platform/session'

function recordingFetch(responses: Response[] = []) {
  const calls: { url: URL; init: RequestInit }[] = []
  const impl: typeof fetch = async (input, init) => {
    calls.push({ url: new URL(String(input)), init: init ?? {} })
    return responses.shift() ?? new Response('{"resourceType":"Patient","id":"p1"}', { status: 200 })
  }
  return { calls, impl }
}

const noAuth: AuthStrategy = { headers: async () => ({}), retryWithNonce: () => false }

describe('httpTransport', () => {
  test('repeats query parameters and parses the body losslessly', async () => {
    const { calls, impl } = recordingFetch([new Response('{"value":2.50}')])
    const response = await httpTransport('https://central.example/api/', noAuth, impl).request({
      method: 'GET',
      path: '/fhir/Observation',
      query: { date: ['ge2026-01-01', 'le2026-12-31'], code: 'x' },
    })
    expect(calls[0]?.url.toString()).toBe('https://central.example/api/fhir/Observation?date=ge2026-01-01&date=le2026-12-31&code=x')
    expect(String((response.body as { value: unknown }).value)).toBe('2.50')
  })

  test('sends a form body for search over POST', async () => {
    const { calls, impl } = recordingFetch()
    await httpTransport('https://edge.example', noAuth, impl).request({ method: 'POST', path: '/fhir/Patient/_search', form: { name: 'a b' } })
    const init = calls[0]?.init
    expect(new Headers(init?.headers).get('content-type')).toBe('application/x-www-form-urlencoded')
    expect(init?.body).toBe('name=a+b')
  })

  test('serializes a JSON body losslessly, keeping a given content type', async () => {
    const { calls, impl } = recordingFetch()
    const { parseJson } = await import('../src/platform/json')
    await httpTransport('https://central.example', noAuth, impl).request({
      method: 'POST',
      path: '/x',
      headers: { 'content-type': 'application/fhir+json' },
      body: parseJson('{"value":1.10}'),
    })
    expect(calls[0]?.init.body).toBe('{"value":1.10}')
    expect(new Headers(calls[0]?.init.headers).get('content-type')).toBe('application/fhir+json')
  })

  test('an empty body is undefined', async () => {
    const { impl } = recordingFetch([new Response(null, { status: 204 })])
    const response = await httpTransport('https://central.example', noAuth, impl).request({ method: 'DELETE', path: '/x' })
    expect(response).toMatchObject({ status: 204, body: undefined })
  })

  test('central sends the session bearer token, and nothing once the session ends', async () => {
    const session = memorySession()
    const { calls, impl } = recordingFetch()
    const transport = httpTransport('https://central.example', bearerAuth(session), impl)
    await transport.request({ method: 'GET', path: '/a' })
    session.setAccessToken('t1')
    await transport.request({ method: 'GET', path: '/a' })
    session.end('logout')
    await transport.request({ method: 'GET', path: '/a' })
    expect(calls.map(({ init }) => new Headers(init.headers).get('authorization'))).toEqual([null, 'Bearer t1', null])
  })

  test('the edge retries once with the nonce a DPoP server asks for', async () => {
    const session = memorySession()
    session.setAccessToken('t1')
    const keys = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign', 'verify'])
    const { calls, impl } = recordingFetch([
      new Response(null, { status: 401, headers: { 'www-authenticate': 'DPoP error="use_dpop_nonce"', 'dpop-nonce': 'n1' } }),
      new Response(null, { status: 401, headers: { 'www-authenticate': 'DPoP error="use_dpop_nonce"', 'dpop-nonce': 'n2' } }),
    ])
    const response = await httpTransport('https://edge.example', dpopAuth(session, () => keys), impl).request({ method: 'GET', path: '/a' })
    expect(response.status).toBe(401)
    expect(calls).toHaveLength(2)
    const second = new Headers(calls[1]?.init.headers).get('dpop') ?? ''
    expect(JSON.parse(atob((second.split('.')[1] ?? '').replace(/-/g, '+').replace(/_/g, '/')))).toMatchObject({ nonce: 'n1' })
  })
})
