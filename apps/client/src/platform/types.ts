// The platform boundary (client-application.md § Composition boundary). Feature code sees only
// these types; how a request travels (HTTP or Tauri IPC) is decided once, in compose.ts.

import type { FhirClient } from './fhir'
import type { OperationClient } from './operations'
import type { RegistriesCommands } from './registries'
import type { SignIn } from './sign-in'

/** Query or form parameters; a list repeats the parameter (`date=ge2026&date=le2027`). */
export type QueryParams = Readonly<Record<string, string | readonly string[]>>

/** One request envelope for every transport. */
export interface ApiRequest {
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  path: string
  query?: QueryParams
  headers?: Record<string, string>
  /** A JSON body, serialized losslessly. */
  body?: unknown
  /** A form body (`application/x-www-form-urlencoded`), for FHIR search over POST. */
  form?: QueryParams
  signal?: AbortSignal
}

/** The response envelope. `body` is parsed losslessly, so decimals keep their source text. */
export interface ApiResponse {
  status: number
  headers: Record<string, string>
  body: unknown
}

export interface Transport {
  request(request: ApiRequest): Promise<ApiResponse>
}

/** Why a session ended: the Query cache is cleared either way (client-application.md § Sessions). */
export type SessionEnd = 'logout' | 'lock'

export interface SessionPort {
  /** The access token for the current user, held in memory only (ADR 0037). */
  accessToken(): string | undefined
  /** Forgets the token and tells every `onEnd` listener. */
  end(reason: SessionEnd): void
  onEnd(listener: (reason: SessionEnd) => void): () => void
}

/** Invalidation hints only: a resource type and id, or a worklist key. */
export interface InvalidationHint {
  resourceType?: string
  id?: string
  worklist?: string
}

export interface EventSourcePort {
  subscribe(listener: (hint: InvalidationHint) => void): () => void
}

export interface Capabilities {
  offlineCapable: boolean
  persistsLocally: boolean
  nativeKeystore: boolean
  printing: boolean
  deviceIntegration: boolean
  syncStatus: boolean
}

/** Which runtime this bundle is serving; read only by the status area, never to branch features. */
export type RuntimeMode = 'central' | 'edge' | 'tauri'

/** The patient portal's own client, session and API transport. */
export interface Portal {
  /** The portal's OAuth client id; the sign-in page asks for a phone for it. */
  clientId: string
  signIn: SignIn
  session: SessionPort
  /** The API (`/api`) with the patient's token. */
  transport: Transport
}

export interface Platform {
  mode: RuntimeMode
  transport: Transport
  /** FHIR reads, searches and operations (client-application.md § Ports). */
  fhir: FhirClient
  /** The runtime's documented HTTP operations, validated at the boundary; command ports build on it. */
  operations: OperationClient
  /** The Registries Bounded Context's commands (registration). */
  registries: RegistriesCommands
  session: SessionPort
  /** Sign-in against the node's authorization server, when runtime.json configures one. */
  signIn?: SignIn
  /** The patient portal (#111), at central when runtime.json configures it: patients sign in
   * with their phone, with a session of their own, never a staff session. */
  portal?: Portal
  events: EventSourcePort
  capabilities: Capabilities
}
