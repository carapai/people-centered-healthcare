import { invoke } from '@tauri-apps/api/core'

import { parseJson, stringifyJson } from './json'
import type { ApiRequest, ApiResponse, Platform } from './types'

interface IpcResponse {
  status: number
  headers: Record<string, string>
  /** JSON text, so numbers stay lossless across the IPC boundary. */
  body: string
}

/** TauriIpcTransport: the same envelope, dispatched in-process into the Axum router (ADR 0036). */
export function tauriPlatform(): Platform {
  return {
    mode: 'tauri',
    transport: {
      async request(request: ApiRequest): Promise<ApiResponse> {
        const response = await invoke<IpcResponse>('api_request', {
          envelope: {
            method: request.method,
            path: request.path,
            query: request.query ?? {},
            headers: request.headers ?? {},
            body: request.body === undefined ? null : stringifyJson(request.body),
          },
        })
        return {
          status: response.status,
          headers: response.headers,
          body: response.body === '' ? undefined : parseJson(response.body),
        }
      },
    },
    // The local desktop session is carried by the Rust side.
    session: { accessToken: () => undefined },
    events: { subscribe: () => () => {} },
    capabilities: {
      offlineCapable: true,
      persistsLocally: true,
      nativeKeystore: true,
      printing: true,
      deviceIntegration: true,
      syncStatus: true,
    },
  }
}
