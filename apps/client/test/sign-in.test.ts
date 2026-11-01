import { describe, expect, test, vi } from 'vitest'

import { memorySession } from '../src/platform/session'
import { SignInError, signIn } from '../src/platform/sign-in'

const ORIGIN = 'https://central.example'
const ISSUER = `${ORIGIN}/auth`

function harness(responses: Response[]) {
  const store = new Map<string, string>()
  const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v), removeItem: (k: string) => void store.delete(k) }
  const calls: { url: string; body: string }[] = []
  const fetchImpl: typeof fetch = async (input, init) => {
    calls.push({ url: String(input), body: String(init?.body ?? '') })
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
})
