import type {
  Bundle,
  OperationOutcome,
  Parameters,
  Patient,
  QuestionnaireResponse,
  ResourceTypeMap,
} from '@health/fhir-types'
import { LosslessNumber } from 'lossless-json'

import type { ApiResponse, QueryParams, Transport } from './types'

export type ResourceType = keyof ResourceTypeMap

export interface RequestOptions {
  signal?: AbortSignal
}

export interface SearchOptions extends RequestOptions {
  /** POST `_search` keeps parameters out of URLs and logs; GET is the default. */
  via?: 'GET' | 'POST'
}

/** Instance, type or system history. */
export type HistoryTarget = { type: ResourceType; id?: string } | { type?: undefined }

export interface MatchOptions extends RequestOptions {
  onlyCertainMatches?: boolean
  count?: number
}

/**
 * The FHIR port (client-application.md § Ports): reads, searches and the operations the UI calls.
 * It never writes; business writes go through the command ports, so the UI never assembles them.
 */
export interface FhirClient {
  read<T extends ResourceType>(type: T, id: string, options?: RequestOptions): Promise<ResourceTypeMap[T]>
  vread<T extends ResourceType>(type: T, id: string, versionId: string, options?: RequestOptions): Promise<ResourceTypeMap[T]>
  search(type: ResourceType, params: QueryParams, options?: SearchOptions): Promise<Bundle>
  history(target: HistoryTarget, params?: QueryParams, options?: RequestOptions): Promise<Bundle>
  /** `Patient/$match`: scored candidates for a Patient (patient-identity.md § API). */
  match(patient: Patient, options?: MatchOptions): Promise<Bundle>
  /** Dry-run `QuestionnaireResponse/$extract`: what completion would create; nothing is stored. */
  extract(response: QuestionnaireResponse, options?: RequestOptions): Promise<Bundle>
  /** `Questionnaire/$populate` with SDC's input parameters: the prefilled response. */
  populate(input: Parameters, options?: RequestOptions): Promise<QuestionnaireResponse>
}

/** A response other than the expected resource. `outcome` is the server's OperationOutcome, if any. */
export class FhirError extends Error {
  constructor(
    readonly status: number,
    readonly outcome: OperationOutcome | undefined,
  ) {
    // Issue codes only: diagnostics can quote clinical data.
    super(`FHIR ${status}${outcome === undefined ? '' : `: ${outcome.issue.map((issue) => issue.code).join(', ')}`}`)
    this.name = 'FhirError'
  }
}

const fhirJson = 'application/fhir+json'

/** `base` is the FHIR base relative to the transport's API base. */
export function fhirClient(transport: Transport, base = '/fhir'): FhirClient {
  async function call<T>(
    expected: string,
    method: 'GET' | 'POST',
    path: string,
    init: { query?: QueryParams; form?: QueryParams; body?: unknown; signal?: AbortSignal | undefined },
  ): Promise<T> {
    const response = await transport.request({
      method,
      path: base + path,
      headers: { accept: fhirJson, ...(init.body === undefined ? {} : { 'content-type': fhirJson }) },
      ...(init.query === undefined ? {} : { query: init.query }),
      ...(init.form === undefined ? {} : { form: init.form }),
      ...(init.body === undefined ? {} : { body: init.body }),
      ...(init.signal === undefined ? {} : { signal: init.signal }),
    })
    return expectResource<T>(response, expected)
  }

  const segment = encodeURIComponent

  return {
    read: (type, id, options) => call(type, 'GET', `/${type}/${segment(id)}`, { signal: options?.signal }),
    vread: (type, id, versionId, options) =>
      call(type, 'GET', `/${type}/${segment(id)}/_history/${segment(versionId)}`, { signal: options?.signal }),
    search: (type, params, options) =>
      options?.via === 'POST'
        ? call('Bundle', 'POST', `/${type}/_search`, { form: params, signal: options.signal })
        : call('Bundle', 'GET', `/${type}`, { query: params, signal: options?.signal }),
    history(target, params, options) {
      const path = target.type === undefined ? '' : `/${target.type}${'id' in target && target.id !== undefined ? `/${segment(target.id)}` : ''}`
      return call('Bundle', 'GET', `${path}/_history`, { query: params ?? {}, signal: options?.signal })
    },
    async match(patient, options) {
      const parameter: NonNullable<Parameters['parameter']> = [{ name: 'resource', resource: patient }]
      if (options?.onlyCertainMatches !== undefined) {
        parameter.push({ name: 'onlyCertainMatches', valueBoolean: options.onlyCertainMatches })
      }
      if (options?.count !== undefined) parameter.push({ name: 'count', valueInteger: new LosslessNumber(String(options.count)) })
      return call('Bundle', 'POST', '/Patient/$match', { body: parameters(parameter), signal: options?.signal })
    },
    async extract(response, options) {
      const result = await call<Bundle | Parameters>('Bundle|Parameters', 'POST', '/QuestionnaireResponse/$extract', {
        body: parameters([{ name: 'questionnaire-response', resource: response }]),
        signal: options?.signal,
      })
      return outParameter<Bundle>(result, 'return', 'Bundle')
    },
    async populate(input, options) {
      const result = await call<Parameters>('Parameters', 'POST', '/Questionnaire/$populate', {
        body: input,
        signal: options?.signal,
      })
      return outParameter<QuestionnaireResponse>(result, 'response', 'QuestionnaireResponse')
    },
  }
}

function parameters(parameter: NonNullable<Parameters['parameter']>): Parameters {
  return { resourceType: 'Parameters', parameter }
}

/** The resource body of a 2xx response whose `resourceType` is one of `expected` (`A|B`). */
function expectResource<T>(response: ApiResponse, expected: string): T {
  const body = response.body as { resourceType?: unknown } | undefined
  const resourceType = typeof body === 'object' && body !== null ? body.resourceType : undefined
  if (response.status >= 200 && response.status < 300 && expected.split('|').includes(resourceType as string)) {
    return body as T
  }
  throw new FhirError(response.status, resourceType === 'OperationOutcome' ? (body as OperationOutcome) : undefined)
}

/**
 * An operation's output: FHIR returns a single resource output bare, otherwise inside
 * Parameters under `name`.
 */
function outParameter<T>(result: { resourceType: string }, name: string, resourceType: string): T {
  if (result.resourceType === resourceType) return result as T
  const resource = (result as Parameters).parameter?.find((p) => p.name === name)?.resource
  if (resource?.resourceType === resourceType) return resource as T
  throw new FhirError(200, undefined)
}
