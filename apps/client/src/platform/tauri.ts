import { invoke } from '@tauri-apps/api/core'

import { fhirClient } from './fhir'
import { formsCommands } from './forms'
import { registriesCommands } from './registries'
import { parseJson, stringifyJson } from './json'
import { operationClient } from './operations'
import { memorySession, type MemorySession } from './session'
import { SignInError, type SignIn } from './sign-in'
import type { ApiRequest, ApiResponse, Platform, Transport } from './types'

export interface IpcResponse {
  status: number
  headers: Record<string, string>
  /** JSON text, so numbers stay lossless across the IPC boundary. */
  body: string
}

/** How a command reaches the Rust side: Tauri's `invoke`, or a stand-in in the contract tests. */
export type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>

/**
 * TauriIpcTransport: the same envelope, dispatched in-process into the Axum router (ADR 0036).
 * The session lives on the Rust side, so no credential is ever put in the envelope.
 */
export function ipcTransport(call: Invoke = invoke): Transport {
  return {
    async request(request: ApiRequest): Promise<ApiResponse> {
      const form = request.form === undefined ? undefined : pairs(request.form)
      const response = await call<IpcResponse>('api_request', {
        envelope: {
          method: request.method,
          path: request.path,
          query: pairs(request.query ?? {}),
          headers: {
            ...request.headers,
            ...(form === undefined ? {} : { 'content-type': 'application/x-www-form-urlencoded' }),
          },
          body:
            form !== undefined
              ? new URLSearchParams(form).toString()
              : request.body === undefined
                ? null
                : stringifyJson(request.body),
        },
      })
      return {
        status: response.status,
        headers: response.headers,
        body: response.body === '' ? undefined : parseJson(response.body),
      }
    },
  }
}

/** Query and form parameters as ordered pairs, so repeated parameters survive IPC. */
function pairs(params: NonNullable<ApiRequest['query']>): [string, string][] {
  return Object.entries(params).flatMap(([name, value]) =>
    (typeof value === 'string' ? [value] : value).map((each): [string, string] => [name, each]),
  )
}

/**
 * Sign-in on the desktop (#134): the credentials go over IPC to the Rust side, which runs the
 * authorization server's flow in process and keeps the session. No token ever reaches the
 * webview; the session here holds only a marker, so the app knows someone is signed in, and the
 * Rust side drops any credential an envelope carries and attaches its own.
 */
export function tauriSignIn(call: Invoke, session: MemorySession, transport: Transport, navigate: (path: string) => void): SignIn {
  return {
    async password(username, password) {
      try {
        await call<null>('sign_in', { username, password })
      } catch (reason) {
        const code = typeof reason === 'string' ? reason : 'unavailable'
        throw new SignInError(code === 'refused' || code === 'facility' ? code : code === 'change_password' ? 'change-password' : 'unavailable')
      }
      session.setAccessToken(SIGNED_IN)
    },
    async signOut() {
      await call<null>('sign_out', {})
      session.end('logout')
    },
    async start(returnTo) {
      navigate(`/sign-in?returnTo=${encodeURIComponent(returnTo)}`)
    },
    async complete() {
      return '/'
    },
    async changePassword(username, password, newPassword) {
      const response = await transport.request({ method: 'POST', path: '/auth/password', form: { username, password, new_password: newPassword } })
      if (response.status >= 200 && response.status < 300) return
      if (response.status === 422) throw new SignInError('rejected')
      throw new SignInError(response.status === 401 ? 'refused' : 'unavailable')
    },
    authorizeEndpoint: '',
  }
}

/** What the desktop's session holds once signed in: a marker, never a token. */
export const SIGNED_IN = 'ipc-session'

export function tauriPlatform(): Platform {
  const transport = ipcTransport()
  const session = memorySession()
  return {
    mode: 'tauri',
    transport,
    fhir: fhirClient(transport),
    registries: registriesCommands(transport),
    forms: formsCommands(transport),
    operations: operationClient(transport),
    // The local desktop session is carried by the Rust side; ending it still clears the cache.
    session,
    signIn: tauriSignIn(invoke, session, transport, (path) => window.location.assign(path)),
    events: { subscribe: () => () => {} },
    capabilities: {
      offlineCapable: true,
      persistsLocally: true,
      nativeKeystore: true,
      printing: true,
      deviceIntegration: true,
      syncStatus: true,
      edgeStatus: false,
    },
  }
}
