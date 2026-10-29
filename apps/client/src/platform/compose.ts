import { httpTransport } from './http'
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
  const session = { accessToken: () => undefined }
  return {
    mode: config.mode,
    transport: httpTransport(config.apiBase, session),
    session,
    events: { subscribe: () => () => {} },
    capabilities: {
      offlineCapable: config.mode === 'edge',
      persistsLocally: false,
      nativeKeystore: false,
      printing: true,
      deviceIntegration: false,
      syncStatus: config.mode === 'edge',
    },
  }
}
