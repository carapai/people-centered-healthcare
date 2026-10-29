import { parseJson, stringifyJson } from './json'
import type { ApiRequest, ApiResponse, SessionPort, Transport } from './types'

/** HttpTransport (client-application.md § Transports): central or edge over HTTPS. */
export function httpTransport(apiBase: string, session: SessionPort): Transport {
  return {
    async request(request: ApiRequest): Promise<ApiResponse> {
      const url = new URL(apiBase.replace(/\/$/, '') + request.path, window.location.origin)
      for (const [key, value] of Object.entries(request.query ?? {})) {
        url.searchParams.set(key, value)
      }
      const headers = new Headers(request.headers)
      headers.set('accept', 'application/fhir+json, application/json')
      const token = session.accessToken()
      if (token !== undefined) {
        headers.set('authorization', `Bearer ${token}`)
      }
      let body: string | undefined
      if (request.body !== undefined) {
        headers.set('content-type', 'application/json')
        body = stringifyJson(request.body)
      }
      const response = await fetch(url, { method: request.method, headers, ...(body === undefined ? {} : { body }) })
      const text = await response.text()
      return {
        status: response.status,
        headers: Object.fromEntries(response.headers.entries()),
        body: text === '' ? undefined : parseJson(text),
      }
    },
  }
}
