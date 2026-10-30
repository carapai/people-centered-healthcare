import type { OperationId, ResponseBody, operations } from '@health/api-client'

import type { QueryParams, Transport } from './types'

type Responses<O extends OperationId> = (typeof operations)[O]['responses']

export interface OperationInput {
  /** Values for the path's `{name}` segments. */
  path?: Record<string, string>
  query?: QueryParams
  body?: unknown
  signal?: AbortSignal
}

/** A documented response: its status and its body, validated against the operation's schema. */
export interface OperationResult<O extends OperationId> {
  status: number
  body: ResponseBody<O, keyof Responses<O>>
}

/**
 * Calls the runtime's OpenAPI operations (client-application.md § Types). Every response is
 * checked with `parseResponse` at this boundary, so an undocumented status or a body that does not
 * match its schema throws a `ResponseError`. The command ports of each Bounded Context are built
 * on this as their server use cases land. The schemas (and Zod) load with the first call, not
 * with the app shell.
 */
export interface OperationClient {
  call<O extends OperationId>(operation: O, input?: OperationInput): Promise<OperationResult<O>>
}

export function operationClient(transport: Transport): OperationClient {
  return {
    async call(operation, input = {}) {
      const { operations, parseResponse } = await import('@health/api-client')
      const { method, path } = operations[operation]
      const response = await transport.request({
        method,
        path: path.replace(/\{(\w+)\}/g, (_, name: string) => {
          const value = input.path?.[name]
          if (value === undefined) throw new TypeError(`${operation}: no value for {${name}}`)
          return encodeURIComponent(value)
        }),
        headers: { accept: 'application/json' },
        ...(input.query === undefined ? {} : { query: input.query }),
        ...(input.body === undefined ? {} : { body: input.body }),
        ...(input.signal === undefined ? {} : { signal: input.signal }),
      })
      return { status: response.status, body: parseResponse(operation, response.status, response.body) }
    },
  }
}
