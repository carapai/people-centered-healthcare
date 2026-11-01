import { bearerAuth, dpopAuth } from './auth'
import { sseEventSource } from './events'
import { fhirClient } from './fhir'
import { registriesCommands } from './registries'
import { httpTransport } from './http'
import { operationClient } from './operations'
import { memorySession } from './session'
import { checkVersion, registerShellWorker } from './shell'
import { type SignInConfig, signIn } from './sign-in'
import type { Platform, RuntimeMode } from './types'

interface RuntimeConfig {
  mode: Exclude<RuntimeMode, 'tauri'>
  apiBase: string
  /** The node's authorization server, for sign-in. */
  auth?: SignInConfig
  /** The FHIR base on this origin (central serves `/fhir`); without it, `{apiBase}/fhir`. */
  fhirBase?: string
  /** The national ID identifier system the registry matches on. */
  ninSystem?: string
  /** The patient portal's client (#111), signing in at `auth.issuer`. */
  portal?: { clientId: string; redirectPath: string }
  /** The build the node serves (an edge's embedded bundle), for the version check. */
  build?: string
}

function isSignInConfig(value: unknown): value is SignInConfig {
  if (typeof value !== 'object' || value === null) return false
  const { issuer, clientId, redirectPath } = value as Record<string, unknown>
  return typeof issuer === 'string' && typeof clientId === 'string' && typeof redirectPath === 'string'
}

function isRuntimeConfig(value: unknown): value is RuntimeConfig {
  if (typeof value !== 'object' || value === null) return false
  const { mode, apiBase, auth, fhirBase, ninSystem, portal, build } = value as Record<string, unknown>
  const portalOk =
    portal === undefined ||
    (typeof portal === 'object' &&
      portal !== null &&
      typeof (portal as Record<string, unknown>)['clientId'] === 'string' &&
      typeof (portal as Record<string, unknown>)['redirectPath'] === 'string')
  return (
    portalOk &&
    (mode === 'central' || mode === 'edge') &&
    typeof apiBase === 'string' &&
    (auth === undefined || isSignInConfig(auth)) &&
    (fhirBase === undefined || typeof fhirBase === 'string') &&
    (ninSystem === undefined || typeof ninSystem === 'string') &&
    (build === undefined || typeof build === 'string')
  )
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
  // A shell cached from before an edge update reloads; the page never composes meanwhile.
  if (!checkVersion(config.build, import.meta.env.VITE_HEALTH_BUILD)) return new Promise(() => {})
  if (config.mode === 'edge') registerShellWorker()
  return httpPlatform(config.mode, config.apiBase, fetch, config.auth, config.fhirBase, config.ninSystem, config.portal)
}

/**
 * Central: a bearer token from the OIDC session. Edge: tokens DPoP-bound to the enrolled device
 * key, which enrollment (#74) provides; until then requests carry no credentials.
 */
export function httpPlatform(
  mode: RuntimeConfig['mode'],
  apiBase: string,
  fetchImpl: typeof fetch = fetch,
  signInConfig?: SignInConfig,
  fhirBase?: string,
  ninSystem?: string,
  portalConfig?: RuntimeConfig['portal'],
): Platform {
  const session = memorySession()
  const auth = mode === 'central' ? bearerAuth(session) : dpopAuth(session, () => undefined)
  const transport = httpTransport(apiBase, auth, fetchImpl)
  // FHIR is served beside the API (central: `/fhir`), or under it when runtime.json names no base.
  const fhirTransport = fhirBase === undefined ? transport : httpTransport('', auth, fetchImpl)
  const fhirPath = fhirBase?.replace(/\/$/, '') ?? '/fhir'
  // The portal is central's only; its patients have a session apart from staff sessions.
  const portal = (() => {
    if (portalConfig === undefined || signInConfig === undefined || mode !== 'central') return undefined
    const portalSession = memorySession()
    const config = { issuer: signInConfig.issuer, clientId: portalConfig.clientId, redirectPath: portalConfig.redirectPath }
    return {
      clientId: portalConfig.clientId,
      session: portalSession,
      signIn: signIn(config, portalSession, { fetch: fetchImpl }),
      transport: httpTransport(apiBase, bearerAuth(portalSession), fetchImpl),
    }
  })()
  return {
    mode,
    transport,
    fhir: fhirClient(fhirTransport, fhirPath),
    operations: operationClient(transport),
    registries: registriesCommands(fhirTransport, fhirPath, ninSystem),
    session,
    // Edges sign in with DPoP-bound tokens once devices are enrolled (#74, #75).
    ...(signInConfig === undefined || mode !== 'central' ? {} : { signIn: signIn(signInConfig, session, { fetch: fetchImpl }) }),
    ...(portal === undefined ? {} : { portal }),
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
