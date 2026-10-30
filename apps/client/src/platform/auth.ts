import type { SessionPort } from './types'

/**
 * How HttpTransport authorizes a request (client-application.md § Transports): a bearer token
 * from central's OIDC session, or an edge token DPoP-bound to the enrolled device key (ADR 0009).
 */
export interface AuthStrategy {
  /** The headers that authorize `method` on `url`; none without a token. */
  headers(method: string, url: URL): Promise<Record<string, string>>
  /**
   * Reads a response's DPoP nonce. True when the server refused the request for want of one,
   * so the transport retries it once with the nonce.
   */
  retryWithNonce(response: Response): boolean
}

export function bearerAuth(session: SessionPort): AuthStrategy {
  return {
    async headers() {
      const token = session.accessToken()
      return token === undefined ? {} : { authorization: `Bearer ${token}` }
    },
    retryWithNonce: () => false,
  }
}

/**
 * RFC 9449 DPoP. `deviceKey` returns the enrolled device's ES256 key pair (a non-extractable
 * private key; enrollment stores it, #74), or nothing before enrollment.
 */
export function dpopAuth(session: SessionPort, deviceKey: () => CryptoKeyPair | undefined): AuthStrategy {
  let nonce: string | undefined
  const jwks = new WeakMap<CryptoKey, Promise<JsonWebKey>>()

  function publicJwk(key: CryptoKey): Promise<JsonWebKey> {
    let jwk = jwks.get(key)
    if (jwk === undefined) {
      jwk = crypto.subtle.exportKey('jwk', key).then(({ kty, crv, x, y }) => ({ kty, crv, x, y }) as JsonWebKey)
      jwks.set(key, jwk)
    }
    return jwk
  }

  return {
    async headers(method, url) {
      const token = session.accessToken()
      const keys = deviceKey()
      if (token === undefined || keys === undefined) return {}
      const header = { typ: 'dpop+jwt', alg: 'ES256', jwk: await publicJwk(keys.publicKey) }
      const claims = {
        jti: crypto.randomUUID(),
        htm: method,
        // The URI without query and fragment (RFC 9449 § 4.2).
        htu: url.origin + url.pathname,
        iat: Math.floor(Date.now() / 1000),
        ath: base64url(new Uint8Array(await crypto.subtle.digest('SHA-256', utf8(token)))),
        ...(nonce === undefined ? {} : { nonce }),
      }
      const input = `${base64url(utf8(JSON.stringify(header)))}.${base64url(utf8(JSON.stringify(claims)))}`
      const signature = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, keys.privateKey, utf8(input))
      return { authorization: `DPoP ${token}`, dpop: `${input}.${base64url(new Uint8Array(signature))}` }
    },
    retryWithNonce(response) {
      const next = response.headers.get('dpop-nonce')
      if (next === null) return false
      const refused = response.status === 401 && /error="use_dpop_nonce"/.test(response.headers.get('www-authenticate') ?? '')
      nonce = next
      return refused
    },
  }
}

function utf8(text: string): Uint8Array<ArrayBuffer> {
  return new TextEncoder().encode(text)
}

export function base64url(bytes: Uint8Array): string {
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}
