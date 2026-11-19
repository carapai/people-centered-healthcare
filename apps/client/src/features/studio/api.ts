import type { StudioActionInput, StudioActionName, StudioPackage, StudioPackagePage, StudioProblem } from '@health/api-client'

import type { Transport } from '../../platform/types'

// The Studio's authoring API (studio.md § The authoring API; #492): `/api/studio` at central,
// acting as the signed-in account. The server derives which actions the person may take, and
// the reason for each one refused, from the lifecycle's rules; this module holds none of them.
// Every response is checked against central's OpenAPI schemas (`@health/api-client`).

export type { StudioActionName, StudioPackage, StudioPackagePage } from '@health/api-client'

/** A request the server refused: its status, its detail and, for a refused step, why. */
export class StudioError extends Error {
  constructor(
    readonly status: number,
    readonly detail: string = '',
    readonly refusal: StudioProblem['refusal'] = null,
  ) {
    super(`studio request failed: ${status}`)
    this.name = 'StudioError'
  }
}

export const studioKey = ['studio', 'packages'] as const

type Operation = 'listStudioPackages' | 'getStudioPackage' | 'takeStudioAction'

/** `body` checked against `operation`'s documented response for `status`; a refusal throws. */
async function parsed<T>(operation: Operation, status: number, body: unknown): Promise<T> {
  // The schemas (and Zod) load with the first Studio request, not with the app shell.
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse(operation, status, body)
  if (status === 200) return value as T
  const problem = value as StudioProblem | null
  throw new StudioError(status, problem?.detail ?? '', problem?.refusal ?? null)
}

/** The package list: one row per package, with the signed-in person's actions. */
export function packagesQuery(transport: Transport) {
  return {
    queryKey: studioKey,
    queryFn: async ({ signal }: { signal?: AbortSignal } = {}): Promise<StudioPackage[]> => {
      const response = await transport.request({ method: 'GET', path: '/studio/packages', ...(signal === undefined ? {} : { signal }) })
      const list = await parsed<{ packages: StudioPackage[] }>('listStudioPackages', response.status, response.body)
      return list.packages
    },
  }
}

const packagePath = (id: string) => `/studio/packages/${encodeURIComponent(id)}`

/** One package version (`name@version`), read from top to bottom. */
export function packageQuery(transport: Transport, id: string) {
  return {
    queryKey: [...studioKey, id] as const,
    queryFn: async ({ signal }: { signal?: AbortSignal } = {}): Promise<StudioPackagePage> => {
      const response = await transport.request({ method: 'GET', path: packagePath(id), ...(signal === undefined ? {} : { signal }) })
      return parsed<StudioPackagePage>('getStudioPackage', response.status, response.body)
    },
  }
}

/** Takes `action` on package version `id`; the server checks it again and answers the page as
 * it is afterwards. */
export async function takeAction(transport: Transport, id: string, action: StudioActionName, input: StudioActionInput = {}): Promise<StudioPackagePage> {
  const response = await transport.request({ method: 'POST', path: `${packagePath(id)}/${action}`, body: input })
  return parsed<StudioPackagePage>('takeStudioAction', response.status, response.body)
}

/** What a step needs besides who takes it: request changes asks why, activate asks where. */
export function needs(action: StudioActionName): 'reason' | 'scope' | null {
  return action === 'request-changes' ? 'reason' : action === 'activate' ? 'scope' : null
}
