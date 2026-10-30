import { bearerAuth, dpopAuth } from './auth'
import { sseEventSource } from './events'
import { fhirClient } from './fhir'
import { httpTransport } from './http'
import { operationClient } from './operations'
import { memorySession } from './session'
import type { Platform, RuntimeMode } from './types'

interface RuntimeConfig {
  mode: Exclude<RuntimeMode, 'tauri'>
  apiBase: string
}

function isRuntimeConfig(value: unknown): value is RuntimeConfig {
  if (typeof value !== 'object' || value === null) return false
  const { mode, apiBase } = value as Record<string, unknown>
  return (mode === 'central' || mode === 'edge') && typeof apiBase === 'string'
}

/**
 * Builds the one Platform for this runtime (client-application.md § Composition boundary).
 * The Tauri build is a Vite mode, so the browser bundle never contains the Tauri code; browser
 * builds read runtime.json from whichever host (central or edge) served them.
 */
export async function composePlatform(): Promise<Platform> {
  if (import.meta.env.MODE === 'tauri') {
    const { tauriPlatform } = await import('./tauri')
    return tauriPlatform()
  }
  const response = await fetch('/runtime.json', { cache: 'no-store' })
  const config: unknown = await response.json()
  if (!isRuntimeConfig(config)) {
    throw new Error('runtime.json must contain a mode (central or edge) and an apiBase')
  }
  return httpPlatform(config.mode, config.apiBase)
}

/**
 * Central: a bearer token from the OIDC session. Edge: tokens DPoP-bound to the enrolled device
 * key, which enrollment (#74) provides; until then requests carry no credentials.
 */
export function httpPlatform(mode: RuntimeConfig['mode'], apiBase: string, fetchImpl: typeof fetch = fetch): Platform {
  const session = memorySession()
  const auth = mode === 'central' ? bearerAuth(session) : dpopAuth(session, () => undefined)
  const transport = httpTransport(apiBase, auth, fetchImpl)
  return {
    mode,
    transport,
    fhir: fhirClient(transport),
    operations: operationClient(transport),
    session,
    events: sseEventSource(`${apiBase.replace(/\/$/, '')}/events`, auth, { fetch: fetchImpl }),
    capabilities: {
      offlineCapable: mode === 'edge',
      persistsLocally: false,
      nativeKeystore: false,
      printing: true,
      deviceIntegration: false,
      syncStatus: mode === 'edge',
    },
  }
}
