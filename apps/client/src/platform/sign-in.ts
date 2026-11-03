import { base64url } from './auth'
import type { MemorySession } from './session'

/** Where the authorization server is, from runtime.json. */
export interface SignInConfig {
  /** The issuer URL, absolute or relative to the page (e.g. `/auth`). */
  issuer: string
  /** This client's id. */
  clientId: string
  /** The redirect URI path registered for this client (e.g. `/callback`). */
  redirectPath: string
}

/**
 * The OAuth transaction kept across the redirect to the authorization server. It is not
 * clinical data, so it may sit in sessionStorage (scripts/check-boundaries.ts allows this file),
 * and it is removed as soon as the callback reads it.
 */
interface Transaction {
  verifier: string
  state: string
  returnTo: string
}

/** Sign-in with authorization code and PKCE (offline-auth.md), keeping tokens in memory only. */
export interface SignIn {
  /** Leaves for the authorization server's sign-in, coming back to `returnTo` afterwards. */
  start(returnTo: string): Promise<void>
  /** Completes the sign-in at the callback URL; returns where to go next. */
  complete(callbackUrl: string): Promise<string>
  /** Revokes the session at the server and forgets the tokens. */
  signOut(): Promise<void>
  /** The authorization endpoint the sign-in page posts the credentials to. */
  readonly authorizeEndpoint: string
  /**
   * Replaces `username`'s password (offline-auth.md § Offline user and role administration): the
   * temporary one an admin set must be changed before the first sign-in. Throws
   * `SignInError('refused')` on a wrong current password, `SignInError('rejected')` when the new
   * one fails the password policy or is a known breached password.
   */
  changePassword(username: string, password: string, newPassword: string): Promise<void>
  /**
   * On an edge (offline-auth.md § Tokens and sessions): locks the session after idling. Its tokens
   * are forgotten and the query cache cleared (`end('lock')`); the session lives on at the edge.
   */
  lock?(): void
  /**
   * Resumes this device's session for `username`, or switches the device to that user's live
   * session here, with their PIN. Throws `SignInError('refused')` on a wrong PIN.
   */
  unlock?(username: string, pin: string): Promise<void>
  /** Sets the signed-in user's 6-digit PIN on this device. */
  setPin?(pin: string): Promise<void>
}

/** Why a sign-in could not complete. */
export class SignInError extends Error {
  constructor(readonly reason: 'state' | 'issuer' | 'refused' | 'rejected' | 'locked' | 'no-transaction' | 'unavailable') {
    super(`sign-in failed: ${reason}`)
    this.name = 'SignInError'
  }
}

const KEY = 'health.sign-in'

export interface SignInOptions {
  fetch?: typeof fetch
  storage?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>
  navigate?: (url: string) => void
  /** The page origin, for absolute redirect URIs (window.location.origin). */
  origin?: string
  /** Seconds before expiry to refresh. */
  refreshMargin?: number
  setTimer?: (callback: () => void, ms: number) => unknown
  clearTimer?: (timer: unknown) => void
  /**
   * A DPoP proof for a request to the authorization server, bound to `token` when given: on an
   * edge, by the device's key, so tokens are bound to it and issued only to an enrolled device
   * (ADR 0009). With it, the session can lock and be unlocked with a PIN.
   */
  proof?: (method: string, url: URL, token?: string) => Promise<string | undefined>
}

function random(bytes: number): string {
  return base64url(crypto.getRandomValues(new Uint8Array(bytes)))
}

async function challenge(verifier: string): Promise<string> {
  return base64url(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier))))
}

export function signIn(config: SignInConfig, session: MemorySession, options: SignInOptions = {}): SignIn {
  const fetchImpl = options.fetch ?? fetch
  const storage = options.storage ?? globalThis.sessionStorage
  const navigate = options.navigate ?? ((url: string) => globalThis.location.assign(url))
  const origin = options.origin ?? globalThis.location.origin
  const setTimer = options.setTimer ?? ((callback: () => void, ms: number) => setTimeout(callback, ms))
  const clearTimer = options.clearTimer ?? ((timer: unknown) => clearTimeout(timer as ReturnType<typeof setTimeout>))
  const margin = options.refreshMargin ?? 60
  const issuer = new URL(config.issuer, origin).toString().replace(/\/$/, '')
  const redirectUri = new URL(config.redirectPath, origin).toString()
  // The refresh token lives in this closure only.
  let refreshToken: string | undefined
  let timer: unknown

  async function post(path: string, form: Record<string, string>, token?: string): Promise<Response> {
    const url = new URL(`${issuer}${path}`)
    const headers: Record<string, string> = { 'content-type': 'application/x-www-form-urlencoded' }
    const proof = await options.proof?.('POST', url, token)
    if (proof !== undefined) headers['dpop'] = proof
    if (token !== undefined) headers['authorization'] = `DPoP ${token}`
    return fetchImpl(url.toString(), { method: 'POST', headers, body: new URLSearchParams(form).toString() })
  }

  async function tokens(form: Record<string, string>, path = '/token'): Promise<void> {
    const response = await post(path, form)
    if (!response.ok) {
      const error = (await response.json().catch(() => ({}))) as { error?: string }
      throw new SignInError(error.error === 'session_locked' ? 'locked' : 'refused')
    }
    const body = (await response.json()) as { access_token: string; refresh_token?: string; expires_in: number }
    session.setAccessToken(body.access_token)
    refreshToken = body.refresh_token
    if (timer !== undefined) clearTimer(timer)
    if (refreshToken !== undefined) {
      timer = setTimer(() => void refresh(), Math.max(body.expires_in - margin, 1) * 1000)
    }
  }

  async function refresh(): Promise<void> {
    if (refreshToken === undefined) return
    try {
      await tokens({ grant_type: 'refresh_token', refresh_token: refreshToken, client_id: config.clientId })
    } catch (error) {
      // Idle at the edge: locked, not ended; the PIN resumes it.
      if (error instanceof SignInError && error.reason === 'locked') {
        lock()
        return
      }
      // The session hit its cap or was revoked: sign out locally.
      refreshToken = undefined
      session.end('logout')
    }
  }

  function lock(): void {
    if (timer !== undefined) clearTimer(timer)
    timer = undefined
    refreshToken = undefined
    session.end('lock')
  }

  // Only where requests are proved with the device's key: an edge.
  const pinned =
    options.proof === undefined
      ? {}
      : {
          lock,
          async unlock(username: string, pin: string) {
            await tokens({ username, pin, client_id: config.clientId }, '/unlock')
          },
          async setPin(pin: string) {
            const token = session.accessToken()
            if (token === undefined) throw new SignInError('refused')
            const response = await post('/pin', { pin }, token)
            if (!response.ok) throw new SignInError('refused')
          },
        }

  return {
    ...pinned,
    authorizeEndpoint: `${issuer}/authorize`,

    async changePassword(username, password, newPassword) {
      const response = await fetchImpl(`${issuer}/password`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ username, password, new_password: newPassword }).toString(),
      })
      if (response.ok) return
      if (response.status === 422) throw new SignInError('rejected')
      throw new SignInError(response.status === 401 ? 'refused' : 'unavailable')
    },

    async start(returnTo) {
      const verifier = random(32)
      const state = random(16)
      const transaction: Transaction = { verifier, state, returnTo }
      storage.setItem(`${KEY}.${config.clientId}`, JSON.stringify(transaction))
      const query = new URLSearchParams({
        response_type: 'code',
        client_id: config.clientId,
        redirect_uri: redirectUri,
        scope: 'openid fhirUser user/*.rs',
        state,
        code_challenge: await challenge(verifier),
        code_challenge_method: 'S256',
      })
      navigate(`${issuer}/authorize?${query.toString()}`)
    },

    async complete(callbackUrl) {
      const params = new URL(callbackUrl, origin).searchParams
      const saved = storage.getItem(`${KEY}.${config.clientId}`)
      storage.removeItem(`${KEY}.${config.clientId}`)
      if (saved === null) throw new SignInError('no-transaction')
      const transaction = JSON.parse(saved) as Transaction
      if (params.get('state') !== transaction.state) throw new SignInError('state')
      // RFC 9207: the answer must come from the issuer we sent the user to.
      if (params.get('iss') !== issuer) throw new SignInError('issuer')
      const code = params.get('code')
      if (code === null) throw new SignInError('refused')
      await tokens({
        grant_type: 'authorization_code',
        code,
        redirect_uri: redirectUri,
        client_id: config.clientId,
        code_verifier: transaction.verifier,
      })
      return transaction.returnTo
    },

    async signOut() {
      if (timer !== undefined) clearTimer(timer)
      const token = refreshToken
      refreshToken = undefined
      session.end('logout')
      if (token !== undefined) {
        await post('/revoke', { token, client_id: config.clientId }).catch(() => undefined)
      }
    },
  }
}
