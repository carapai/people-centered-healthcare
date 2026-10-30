import type { SessionEnd, SessionPort } from './types'

/** A browser session: the token lives in this closure only, never in storage (ADR 0037). */
export interface MemorySession extends SessionPort {
  /** Set by the sign-in flow (central OIDC or the edge's authorization server). */
  setAccessToken(token: string): void
}

export function memorySession(): MemorySession {
  let token: string | undefined
  const listeners = new Set<(reason: SessionEnd) => void>()
  return {
    accessToken: () => token,
    setAccessToken(next) {
      token = next
    },
    end(reason) {
      token = undefined
      for (const listener of listeners) listener(reason)
    },
    onEnd(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}
