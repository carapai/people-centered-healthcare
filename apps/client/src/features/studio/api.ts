import type { StudioActionInput, StudioActionName, StudioFile, StudioNewDraft, StudioPackage, StudioPackagePage, StudioProblem } from '@health/api-client'

import type { Transport } from '../../platform/types'

// The Studio's authoring API (studio.md § The authoring API; #492): `/api/studio` at central,
// acting as the signed-in account. The server derives which actions the person may take, and
// the reason for each one refused, from the lifecycle's rules; this module holds none of them.
// Every response is checked against central's OpenAPI schemas (`@health/api-client`).

export type { StudioActionName, StudioFile, StudioNewDraft, StudioPackage, StudioPackageKind, StudioPackagePage } from '@health/api-client'

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

type Operation = 'listStudioPackages' | 'getStudioPackage' | 'takeStudioAction' | 'getStudioFile' | 'saveStudioFile' | 'removeStudioFile' | 'setStudioVersion' | 'setStudioNote' | 'startStudioDraft' | 'commentStudioPackage'

/** `body` checked against `operation`'s documented response for `status`; a refusal throws. */
async function parsed<T>(operation: Operation, status: number, body: unknown): Promise<T> {
  // The schemas (and Zod) load with the first Studio request, not with the app shell.
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse(operation, status, body)
  if (status === 200 || status === 201) return value as T
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

const filePath = (id: string, path: string) => `${packagePath(id)}/files/${path.split('/').map(encodeURIComponent).join('/')}`

/** The revision a change was read at, as the `If-Match` the server checks: a stale one is
 * refused with 412 (studio.md § The authoring API, Editing). */
const ifMatch = (revision: number) => ({ 'if-match': `"${revision}"` })

/** One file of a package version, with the draft's revision when it was read. */
export function fileQuery(transport: Transport, id: string, path: string) {
  return {
    queryKey: [...studioKey, id, 'files', path] as const,
    queryFn: async ({ signal }: { signal?: AbortSignal } = {}): Promise<StudioFile> => {
      const response = await transport.request({ method: 'GET', path: filePath(id, path), ...(signal === undefined ? {} : { signal }) })
      return parsed<StudioFile>('getStudioFile', response.status, response.body)
    },
  }
}

/** Saves one file of draft `id`, read at `revision`; answers the page as it is now. */
export async function saveFile(transport: Transport, id: string, path: string, content: string, revision: number): Promise<StudioPackagePage> {
  const response = await transport.request({ method: 'PUT', path: filePath(id, path), headers: ifMatch(revision), body: { content } })
  return parsed<StudioPackagePage>('saveStudioFile', response.status, response.body)
}

/** Removes one file of draft `id`, read at `revision`. */
export async function removeFile(transport: Transport, id: string, path: string, revision: number): Promise<StudioPackagePage> {
  const response = await transport.request({ method: 'DELETE', path: filePath(id, path), headers: ifMatch(revision) })
  return parsed<StudioPackagePage>('removeStudioFile', response.status, response.body)
}

/** Gives draft `id`, read at `revision`, another version; the page comes back under its new id. */
export async function setVersion(transport: Transport, id: string, version: string, revision: number): Promise<StudioPackagePage> {
  const response = await transport.request({ method: 'PUT', path: `${packagePath(id)}/version`, headers: ifMatch(revision), body: { version } })
  return parsed<StudioPackagePage>('setStudioVersion', response.status, response.body)
}

/** Keeps the holder's one change note for the whole draft; an empty one clears it. */
export async function setNote(transport: Transport, id: string, note: string, revision: number): Promise<StudioPackagePage> {
  const response = await transport.request({ method: 'PUT', path: `${packagePath(id)}/note`, headers: ifMatch(revision), body: { note } })
  return parsed<StudioPackagePage>('setStudioNote', response.status, response.body)
}

/** Adds a comment to the draft's one discussion, naming the artefact at path `about` if it is
 * about one (#496). The server writes the entry; it answers the page with it. */
export async function addComment(transport: Transport, id: string, text: string, about?: string): Promise<StudioPackagePage> {
  const response = await transport.request({ method: 'POST', path: `${packagePath(id)}/comments`, body: { text, ...(about === undefined || about === '' ? {} : { about }) } })
  return parsed<StudioPackagePage>('commentStudioPackage', response.status, response.body)
}

/** Starts a draft: a copy of a published version (`copyOf`), or a blank new package. */
export async function startDraft(transport: Transport, input: StudioNewDraft): Promise<StudioPackagePage> {
  const response = await transport.request({ method: 'POST', path: '/studio/drafts', body: input })
  return parsed<StudioPackagePage>('startStudioDraft', response.status, response.body)
}

/** What a step needs besides who takes it: requesting changes asks why, activating where,
 * handing over to whom, and reassigning to whom and why. */
export function needs(action: StudioActionName): readonly ('reason' | 'scope' | 'to')[] {
  switch (action) {
    case 'request-changes':
      return ['reason']
    case 'activate':
      return ['scope']
    case 'hand-over':
      return ['to']
    case 'reassign':
      return ['to', 'reason']
    default:
      return []
  }
}

/** The body of `action`, from the fields the person filled in. */
export function actionInput(action: StudioActionName, fields: { reason: string; scope: string; to: string }): StudioActionInput {
  const wanted = needs(action)
  return {
    ...(wanted.includes('reason') ? { reason: fields.reason } : {}),
    ...(wanted.includes('scope') ? { scope: fields.scope } : {}),
    ...(wanted.includes('to') ? { to: fields.to.trim() } : {}),
  }
}
