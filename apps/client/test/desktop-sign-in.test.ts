// Sign-in on the desktop (#134): the credentials go over IPC, the session stays on the Rust side,
// and the webview holds only a marker, never a token.

import { describe, expect, test } from 'vitest'

import { memorySession } from '../src/platform/session'
import { SignInError } from '../src/platform/sign-in'
import { type Invoke, SIGNED_IN, tauriSignIn } from '../src/platform/tauri'
import type { ApiRequest, ApiResponse, Transport } from '../src/platform/types'

function harness(answer: (command: string, args: Record<string, unknown>) => unknown) {
  const invoked: { command: string; args: Record<string, unknown> }[] = []
  const call: Invoke = async <T,>(command: string, args: Record<string, unknown>): Promise<T> => {
    invoked.push({ command, args })
    const result = answer(command, args)
    if (result instanceof Error) throw result.message
    return result as T
  }
  const requests: ApiRequest[] = []
  let status = 204
  const transport: Transport = {
    async request(request: ApiRequest): Promise<ApiResponse> {
      requests.push(request)
      return { status, headers: {}, body: undefined }
    },
  }
  const navigated: string[] = []
  const session = memorySession()
  const signIn = tauriSignIn(call, session, transport, (path) => navigated.push(path))
  return { signIn, session, invoked, requests, navigated, setStatus: (next: number) => (status = next) }
}

describe('desktop sign-in', () => {
  test('signs in over IPC and keeps only a marker', async () => {
    const h = harness(() => null)
    await h.signIn.password!('nakato', 'correct horse battery')
    expect(h.invoked).toEqual([{ command: 'sign_in', args: { username: 'nakato', password: 'correct horse battery' } }])
    expect(h.session.accessToken()).toBe(SIGNED_IN)
  })

  test.each([
    ['refused', 'refused'],
    ['facility', 'facility'],
    ['change_password', 'change-password'],
    ['unavailable', 'unavailable'],
    ['anything else', 'unavailable'],
  ] as const)('a refusal %s is SignInError %s, and nobody is signed in', async (answer, reason) => {
    const h = harness(() => new Error(answer))
    const failure = await h.signIn.password!('nakato', 'wrong').catch((caught: unknown) => caught)
    expect(failure).toBeInstanceOf(SignInError)
    expect((failure as SignInError).reason).toBe(reason)
    expect(h.session.accessToken()).toBeUndefined()
  })

  test('signs out over IPC and ends the session', async () => {
    const h = harness(() => null)
    await h.signIn.password!('nakato', 'correct horse battery')
    let ended: string | undefined
    h.session.onEnd((reason) => (ended = reason))
    await h.signIn.signOut()
    expect(h.invoked.at(-1)?.command).toBe('sign_out')
    expect(h.session.accessToken()).toBeUndefined()
    expect(ended).toBe('logout')
  })

  test('start goes to the sign-in page, and a password changes over the API', async () => {
    const h = harness(() => null)
    await h.signIn.start('/registration')
    expect(h.navigated).toEqual(['/sign-in?returnTo=%2Fregistration'])
    await h.signIn.changePassword('nakato', 'temporary', 'a much longer one')
    expect(h.requests[0]).toMatchObject({ method: 'POST', path: '/auth/password', form: { username: 'nakato', password: 'temporary', new_password: 'a much longer one' } })
    h.setStatus(422)
    await expect(h.signIn.changePassword('nakato', 'temporary', 'short')).rejects.toMatchObject({ reason: 'rejected' })
    h.setStatus(401)
    await expect(h.signIn.changePassword('nakato', 'wrong', 'a much longer one')).rejects.toMatchObject({ reason: 'refused' })
  })
})
