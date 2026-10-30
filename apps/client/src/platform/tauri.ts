import { invoke } from '@tauri-apps/api/core'

import { fhirClient } from './fhir'
import { parseJson, stringifyJson } from './json'
import { operationClient } from './operations'
import { memorySession } from './session'
import type { ApiRequest, ApiResponse, Platform, Transport } from './types'

interface IpcResponse {
  status: number
  headers: Record<string, string>
  /** JSON text, so numbers stay lossless across the IPC boundary. */
  body: string
}

/** TauriIpcTransport: the same envelope, dispatched in-process into the Axum router (ADR 0036). */
const ipcTransport: Transport = {
  async request(request: ApiRequest): Promise<ApiResponse> {
    const form = request.form === undefined ? undefined : pairs(request.form)
    const response = await invoke<IpcResponse>('api_request', {
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

/** Query and form parameters as ordered pairs, so repeated parameters survive IPC. */
function pairs(params: NonNullable<ApiRequest['query']>): [string, string][] {
  return Object.entries(params).flatMap(([name, value]) =>
    (typeof value === 'string' ? [value] : value).map((each): [string, string] => [name, each]),
  )
}

export function tauriPlatform(): Platform {
  return {
    mode: 'tauri',
    transport: ipcTransport,
    fhir: fhirClient(ipcTransport),
    operations: operationClient(ipcTransport),
    // The local desktop session is carried by the Rust side; ending it still clears the cache.
    session: memorySession(),
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
