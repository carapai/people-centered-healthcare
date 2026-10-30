import { describe, expect, test } from 'vitest'

import { base64url, dpopAuth } from '../src/platform/auth'
import { memorySession } from '../src/platform/session'

function decode(part: string): unknown {
  return JSON.parse(new TextDecoder().decode(fromBase64url(part)))
}

function fromBase64url(part: string): Uint8Array<ArrayBuffer> {
  const binary = atob(part.replace(/-/g, '+').replace(/_/g, '/'))
  return Uint8Array.from(binary, (char) => char.charCodeAt(0))
}

describe('dpopAuth', async () => {
  const keys = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign', 'verify'])
  const session = memorySession()
  const auth = dpopAuth(session, () => keys)

  test('sends nothing without a token or a device key', async () => {
    expect(await auth.headers('GET', new URL('https://edge.example/a'))).toEqual({})
    session.setAccessToken('t')
    expect(await dpopAuth(session, () => undefined).headers('GET', new URL('https://edge.example/a'))).toEqual({})
  })

  test('signs an RFC 9449 proof bound to the token, method and URI', async () => {
    session.setAccessToken('access-token')
    const headers = await auth.headers('POST', new URL('https://edge.example/fhir/Patient/_search?name=x#f'))
    expect(headers['authorization']).toBe('DPoP access-token')
    const [header = '', claims = '', signature = ''] = (headers['dpop'] ?? '').split('.')
    expect(decode(header)).toEqual({ typ: 'dpop+jwt', alg: 'ES256', jwk: { kty: 'EC', crv: 'P-256', x: expect.any(String), y: expect.any(String) } })
    const ath = base64url(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode('access-token'))))
    expect(decode(claims)).toEqual({
      jti: expect.stringMatching(/^[0-9a-f-]{36}$/),
      htm: 'POST',
      htu: 'https://edge.example/fhir/Patient/_search',
      iat: expect.any(Number),
      ath,
    })
    const valid = await crypto.subtle.verify(
      { name: 'ECDSA', hash: 'SHA-256' },
      keys.publicKey,
      fromBase64url(signature),
      new TextEncoder().encode(`${header}.${claims}`),
    )
    expect(valid).toBe(true)
  })

  test('every proof has a fresh jti', async () => {
    session.setAccessToken('t')
    const url = new URL('https://edge.example/a')
    const jti = async () => (decode((await auth.headers('GET', url))['dpop']?.split('.')[1] ?? '') as { jti: string }).jti
    expect(await jti()).not.toBe(await jti())
  })

  test('a nonce on a successful response is kept without a retry', () => {
    const response = new Response(null, { status: 200, headers: { 'dpop-nonce': 'n' } })
    expect(auth.retryWithNonce(response)).toBe(false)
  })
})
