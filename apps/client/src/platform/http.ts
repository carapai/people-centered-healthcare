import type { AuthStrategy } from './auth'
import { parseJson, stringifyJson } from './json'
import type { ApiRequest, ApiResponse, QueryParams, Transport } from './types'

/**
 * HttpTransport (client-application.md § Transports): central or edge over HTTPS. `apiBase` is
 * absolute or relative to the page; `auth` is OIDC bearer (central) or DPoP (edge).
 */
export function httpTransport(apiBase: string, auth: AuthStrategy, fetchImpl: typeof fetch = fetch): Transport {
  const base = apiBase.replace(/\/$/, '')

  async function send(request: ApiRequest): Promise<Response> {
    const url = new URL(base + request.path, globalThis.location?.origin)
    appendParams(url.searchParams, request.query)
    const headers = new Headers(request.headers)
    if (!headers.has('accept')) headers.set('accept', 'application/fhir+json, application/json')
    for (const [name, value] of Object.entries(await auth.headers(request.method, url))) {
      headers.set(name, value)
    }
    let body: string | undefined
    if (request.form !== undefined) {
      const form = new URLSearchParams()
      appendParams(form, request.form)
      headers.set('content-type', 'application/x-www-form-urlencoded')
      body = form.toString()
    } else if (request.body !== undefined) {
      if (!headers.has('content-type')) headers.set('content-type', 'application/json')
      body = stringifyJson(request.body)
    }
    return fetchImpl(url, {
      method: request.method,
      headers,
      ...(body === undefined ? {} : { body }),
      ...(request.signal === undefined ? {} : { signal: request.signal }),
    })
  }

  return {
    async request(request: ApiRequest): Promise<ApiResponse> {
      let response = await send(request)
      if (auth.retryWithNonce(response)) response = await send(request)
      const text = await response.text()
      return {
        status: response.status,
        headers: Object.fromEntries(response.headers.entries()),
        body: text === '' ? undefined : parseJson(text),
      }
    },
  }
}

function appendParams(target: URLSearchParams, params: QueryParams | undefined): void {
  for (const [name, value] of Object.entries(params ?? {})) {
    for (const each of typeof value === 'string' ? [value] : value) target.append(name, each)
  }
}
