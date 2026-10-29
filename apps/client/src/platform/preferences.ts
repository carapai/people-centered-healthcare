// The only browser storage the client may use: non-clinical UI preferences keyed by user
// (ADR 0037). scripts/check-boundaries.ts rejects storage APIs anywhere else.

const prefix = 'health.preferences.'

export function readPreferences(userId: string): Record<string, unknown> {
  try {
    const raw = window.localStorage.getItem(prefix + userId)
    return raw === null ? {} : (JSON.parse(raw) as Record<string, unknown>)
  } catch {
    return {}
  }
}

export function writePreferences(userId: string, preferences: Record<string, unknown>): void {
  window.localStorage.setItem(prefix + userId, JSON.stringify(preferences))
}
