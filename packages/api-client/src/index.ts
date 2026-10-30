// @health/api-client: Zod schemas and types for each runtime's HTTP API, generated from its
// OpenAPI document (client-application.md § Types; ADR 0036). Transports validate every response
// with `parseResponse` at the boundary.

import type { z } from 'zod'

import { operations } from './central'

export * from './central'
export { decimal, integer } from './numbers'

export type OperationId = keyof typeof operations

type Responses<O extends OperationId> = (typeof operations)[O]['responses']

/** The parsed body of `operation`'s response with `status`. */
export type ResponseBody<O extends OperationId, S extends keyof Responses<O>> =
  Responses<O>[S] extends z.ZodType ? z.output<Responses<O>[S]> : null

/** Thrown when a response's status is not documented or its body does not match its schema. */
export class ResponseError extends Error {
  constructor(
    readonly operation: OperationId,
    readonly status: number,
    readonly issues: readonly z.core.$ZodIssue[],
  ) {
    super(`${operation} ${status}: ${issues.length === 0 ? 'undocumented status' : 'body does not match its schema'}`)
    this.name = 'ResponseError'
  }
}

/**
 * Validates `body` (already parsed with lossless-json) against the schema `operation` documents
 * for `status`. Undocumented statuses and mismatched bodies throw a `ResponseError`; the error
 * carries Zod's issues (paths and codes), never the body.
 */
export function parseResponse<O extends OperationId>(operation: O, status: number, body: unknown): ResponseBody<O, keyof Responses<O>> {
  const responses = operations[operation].responses as Record<number, z.ZodType | null>
  if (!(status in responses)) throw new ResponseError(operation, status, [])
  const schema = responses[status]
  if (schema === null || schema === undefined) return null as ResponseBody<O, keyof Responses<O>>
  const result = schema.safeParse(body)
  if (!result.success) throw new ResponseError(operation, status, result.error.issues)
  return result.data as ResponseBody<O, keyof Responses<O>>
}
