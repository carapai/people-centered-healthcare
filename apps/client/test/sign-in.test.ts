import { describe, expect, test, vi } from 'vitest'

import { memorySession } from '../src/platform/session'
import { SignInError, signIn } from '../src/platform/sign-in'

const ORIGIN = 'https://central.example'
const ISSUER = `${ORIGIN}/auth`

function harness(responses: Response[], proof?: (method: string, url: URL, token?: string) => Promise<string | undefined>) {
  const store = new Map<string, string>()
  const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v), removeItem: (k: string) => void store.delete(k) }
  const calls: { url: string; body: string; dpop: string | undefined; authorization: string | undefined }[] = []
  const fetchImpl: typeof fetch = async (input, init) => {
    const headers = init?.headers as Record<string, string> | undefined
    calls.push({ url: String(input), body: String(init?.body ?? ''), dpop: headers?.['dpop'], authorization: headers?.['authorization'] })
    return responses.shift() ?? new Response('{}', { status: 500 })
  }
  const navigated: string[] = []
  const timers: { callback: () => void; ms: number }[] = []
  const session = memorySession()
  const client = signIn({ issuer: '/auth', clientId: 'health-client', redirectPath: '/callback' }, session, {
    fetch: fetchImpl,
    storage,
    navigate: (url) => navigated.push(url),
    origin: ORIGIN,
    setTimer: (callback, ms) => timers.push({ callback, ms }),
    clearTimer: () => {},
    ...(proof === undefined ? {} : { proof }),
  })
  return { client, session, calls, navigated, timers, store }
}

const tokens = (access: string, refresh: string, expiresIn = 900) =>
  new Response(JSON.stringify({ access_token: access, refresh_token: refresh, expires_in: expiresIn, token_type: 'Bearer' }), { status: 200 })

async function startAndCallback(h: ReturnType<typeof harness>) {
  await h.client.start('/registry')
  const authorize = new URL(h.navigated.at(-1) ?? '')
  const state = authorize.searchParams.get('state') ?? ''
  return { authorize, callback: `${ORIGIN}/callback?code=c1&state=${state}&iss=${encodeURIComponent(ISSUER)}` }
}

describe('signIn', () => {
  test('starts the authorization code flow with an S256 PKCE challenge', async () => {
    const h = harness([])
    const { authorize } = await startAndCallback(h)
    expect(authorize.origin + authorize.pathname).toBe(`${ISSUER}/authorize`)
    expect(authorize.searchParams.get('redirect_uri')).toBe(`${ORIGIN}/callback`)
    expect(authorize.searchParams.get('code_challenge_method')).toBe('S256')
    const saved = JSON.parse(h.store.get('health.sign-in.health-client') ?? '{}') as { verifier: string }
    const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(saved.verifier)))
    const expected = btoa(String.fromCharCode(...digest)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
    expect(authorize.searchParams.get('code_challenge')).toBe(expected)
  })

  test('completes with the verifier, keeps tokens in memory and schedules the refresh', async () => {
    const h = harness([tokens('a1', 'r1'), tokens('a2', 'r2')])
    const { callback } = await startAndCallback(h)
    expect(await h.client.complete(callback)).toBe('/registry')
    expect(h.session.accessToken()).toBe('a1')
    const body = new URLSearchParams(h.calls[0]?.body)
    expect(body.get('grant_type')).toBe('authorization_code')
    expect(body.get('code_verifier')?.length).toBeGreaterThanOrEqual(43)
    expect(h.store.size).toBe(0)
    // The refresh fires a minute before the token expires and rotates the refresh token.
    expect(h.timers[0]?.ms).toBe(840_000)
    h.timers[0]?.callback()
    await vi.waitFor(() => expect(h.session.accessToken()).toBe('a2'))
    expect(new URLSearchParams(h.calls[1]?.body).get('refresh_token')).toBe('r1')
  })

  test('a failed refresh signs out locally', async () => {
    const h = harness([tokens('a1', 'r1'), new Response('{"error":"invalid_grant"}', { status: 400 })])
    const ended: string[] = []
    h.session.onEnd((reason) => ended.push(reason))
    await h.client.complete((await startAndCallback(h)).callback)
    h.timers[0]?.callback()
    await vi.waitFor(() => expect(ended).toEqual(['logout']))
    expect(h.session.accessToken()).toBeUndefined()
  })

  test('a callback with the wrong state or issuer, or no transaction, is refused', async () => {
    const h = harness([tokens('a1', 'r1')])
    const { callback } = await startAndCallback(h)
    await expect(h.client.complete(callback.replace(/state=[^&]+/, 'state=forged'))).rejects.toEqual(new SignInError('state'))
    // The transaction is gone after one attempt.
    await expect(h.client.complete(callback)).rejects.toEqual(new SignInError('no-transaction'))
    const again = await startAndCallback(h)
    await expect(h.client.complete(again.callback.replace(/iss=[^&]+/, 'iss=https%3A%2F%2Fevil.example'))).rejects.toEqual(new SignInError('issuer'))
    expect(h.session.accessToken()).toBeUndefined()
  })

  test('sign-out revokes the refresh token and ends the session', async () => {
    const h = harness([tokens('a1', 'r1'), new Response(null, { status: 200 })])
    await h.client.complete((await startAndCallback(h)).callback)
    await h.client.signOut()
    expect(h.session.accessToken()).toBeUndefined()
    expect(h.calls[1]?.url).toBe(`${ISSUER}/revoke`)
    expect(new URLSearchParams(h.calls[1]?.body).get('token')).toBe('r1')
    expect(h.client.authorizeEndpoint).toBe(`${ISSUER}/authorize`)
  })

  test('on an edge, the token and revoke requests carry the device key’s proof', async () => {
    const proved: string[] = []
    const h = harness([tokens('a1', 'r1'), new Response(null, { status: 200 })], async (method, url) => {
      proved.push(`${method} ${url.toString()}`)
      return 'device-proof'
    })
    const { callback } = await startAndCallback(h)
    await h.client.complete(callback)
    await h.client.signOut()
    expect(proved).toEqual([`POST ${ISSUER}/token`, `POST ${ISSUER}/revoke`])
    expect(h.calls.map((c) => c.dpop)).toEqual(['device-proof', 'device-proof'])
  })

  test('without a proof signer, no DPoP header is sent', async () => {
    const h = harness([tokens('a1', 'r1')])
    const { callback } = await startAndCallback(h)
    await h.client.complete(callback)
    expect(h.calls[0]?.dpop).toBeUndefined()
  })

  test('on an edge, idling at the server locks the session rather than ending it', async () => {
    const locked = new Response(JSON.stringify({ error: 'session_locked' }), { status: 401 })
    const h = harness([tokens('a1', 'r1'), locked], async () => 'device-proof')
    const ended: string[] = []
    h.session.onEnd((reason) => ended.push(reason))
    const { callback } = await startAndCallback(h)
    await h.client.complete(callback)
    h.timers[0]?.callback()
    await vi.waitFor(() => expect(ended).toEqual(['lock']))
    expect(h.session.accessToken()).toBeUndefined()
  })

  test('the PIN unlocks with the device proof, and sets with the token', async () => {
    const proved: (string | undefined)[] = []
    const h = harness([tokens('a1', 'r1'), new Response(null, { status: 204 }), tokens('a2', 'r2')], async (_m, _u, token) => {
      proved.push(token)
      return 'device-proof'
    })
    const { callback } = await startAndCallback(h)
    await h.client.complete(callback)
    await h.client.setPin?.('042137')
    expect(h.calls[1]?.url).toBe(`${ISSUER}/pin`)
    expect(h.calls[1]?.authorization).toBe('DPoP a1')
    expect(new URLSearchParams(h.calls[1]?.body).get('pin')).toBe('042137')

    h.client.lock?.()
    expect(h.session.accessToken()).toBeUndefined()
    await h.client.unlock?.('nakato', '042137')
    expect(h.calls[2]?.url).toBe(`${ISSUER}/unlock`)
    expect(Object.fromEntries(new URLSearchParams(h.calls[2]?.body))).toEqual({ username: 'nakato', pin: '042137', client_id: 'health-client' })
    expect(h.calls[2]?.authorization).toBeUndefined()
    expect(h.session.accessToken()).toBe('a2')
    // The PIN request's proof is bound to the token; the unlock's to none.
    expect(proved).toEqual([undefined, 'a1', undefined])
  })

  test('a temporary password is changed at /password; a wrong one is refused, a weak new one rejected', async () => {
    const h = harness([new Response(null, { status: 204 }), new Response('{}', { status: 401 }), new Response('{"error":"password_rejected"}', { status: 422 })])
    await h.client.changePassword('auma', 'temporary pass 1', 'river lantern copper 42')
    expect(h.calls[0]?.url).toBe(`${ISSUER}/password`)
    expect(Object.fromEntries(new URLSearchParams(h.calls[0]?.body))).toEqual({ username: 'auma', password: 'temporary pass 1', new_password: 'river lantern copper 42' })
    await expect(h.client.changePassword('auma', 'wrong', 'river lantern copper 42')).rejects.toEqual(new SignInError('refused'))
    await expect(h.client.changePassword('auma', 'temporary pass 1', 'password')).rejects.toEqual(new SignInError('rejected'))
    // Nothing about it signs the user in.
    expect(h.session.accessToken()).toBeUndefined()
  })

  test('a wrong PIN is refused, and central (no proof) has no PIN at all', async () => {
    const h = harness([new Response(JSON.stringify({ error: 'invalid_grant' }), { status: 401 })], async () => 'device-proof')
    await expect(h.client.unlock?.('nakato', '999999')).rejects.toEqual(new SignInError('refused'))
    expect(harness([]).client.unlock).toBeUndefined()
  })
})
