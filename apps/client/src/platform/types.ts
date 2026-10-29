// The platform boundary (client-application.md § Composition boundary). Feature code sees only
// these types; how a request travels (HTTP or Tauri IPC) is decided once, in compose.ts.

/** One request envelope for every transport. */
export interface ApiRequest {
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  path: string
  query?: Record<string, string>
  headers?: Record<string, string>
  body?: unknown
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

export interface SessionPort {
  /** The access token for the current user, held in memory only (ADR 0037). */
  accessToken(): string | undefined
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

export interface Platform {
  mode: RuntimeMode
  transport: Transport
  session: SessionPort
  events: EventSourcePort
  capabilities: Capabilities
}
