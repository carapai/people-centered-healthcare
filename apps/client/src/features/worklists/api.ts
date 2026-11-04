import { worklistKey } from '../../platform'
import type { Transport } from '../../platform/types'

/** A definition this node runs, with its human tasks (`GET /api/workflow/definitions`). */
export interface DefinitionSummary {
  /** `url|version`. */
  definition: string
  tasks: { state: string; role: string; worklist: string }[]
}

/** A Task projected from a workflow instance (#99), with the events its state takes. */
export interface WorkTask {
  id: string
  instance: string
  definition: string
  context: string
  /** The engine state: the Task's businessStatus. */
  state: string
  status: 'ready' | 'in-progress' | 'completed' | 'cancelled'
  role: string
  form: string | null
  worklist: string
  priority: 'routine' | 'urgent' | 'asap' | 'stat'
  owner: string | null
  authoredOn: string
  lastModified: string
  /** What completing this step can be, e.g. `triaged`; empty once the task is closed. */
  events: string[]
}

/** A request the server refused, with its status and reason. */
export class WorkflowError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`workflow request failed: ${status}`)
    this.name = 'WorkflowError'
  }
}

const reason = (body: unknown) => (typeof body === 'object' && body !== null && typeof (body as { error?: unknown }).error === 'string' ? (body as { error: string }).error : '')

async function call<T>(transport: Transport, method: 'GET' | 'POST', path: string, ok: number[], body?: unknown): Promise<T> {
  const response = await transport.request({ method, path: `/workflow${path}`, ...(body === undefined ? {} : { body }) })
  if (!ok.includes(response.status)) throw new WorkflowError(response.status, reason(response.body))
  return response.body as T
}

/** Every command carries a fresh op id, so the server treats a retry of it as done. */
const opId = () => crypto.randomUUID()

export const DEFINITIONS = ['workflow-definitions'] as const

export function definitionsQuery(transport: Transport) {
  return {
    queryKey: DEFINITIONS,
    queryFn: () => call<DefinitionSummary[]>(transport, 'GET', '/definitions', [200]),
  }
}

/** The worklists the node's definitions name, each once, sorted. */
export function worklists(definitions: DefinitionSummary[]): string[] {
  const keys = [...new Set(definitions.flatMap((d) => d.tasks.map((t) => t.worklist)))]
  // A fresh array, so sorting it in place mutates nothing shared (`toSorted` is ES2023).
  // oxlint-disable-next-line unicorn/no-array-sort
  return keys.sort()
}

export const worklistQueryKey = (key: string) => worklistKey(`workflow:${key}`)

/** The open tasks on `key`, most urgent first, then by arrival. */
export function worklistQuery(transport: Transport, key: string) {
  return {
    queryKey: worklistQueryKey(key),
    queryFn: () => call<WorkTask[]>(transport, 'GET', `/worklists/${encodeURIComponent(key)}`, [200]),
  }
}

export function startInstance(transport: Transport, definition: string, context: string) {
  return call<{ id: string }>(transport, 'POST', '/instances', [200, 201], { definition, context, opId: opId() })
}

const taskPath = (task: WorkTask, command: string) => `/instances/${encodeURIComponent(task.instance)}/tasks/${encodeURIComponent(task.state)}/${command}`

export function claim(transport: Transport, task: WorkTask) {
  return call(transport, 'POST', taskPath(task, 'claim'), [200], { opId: opId() })
}

export function release(transport: Transport, task: WorkTask) {
  return call(transport, 'POST', taskPath(task, 'release'), [200], { opId: opId() })
}

/** Completes the task's step with one of its `events`. */
export function complete(transport: Transport, task: WorkTask, event: string) {
  return call(transport, 'POST', `/instances/${encodeURIComponent(task.instance)}/events`, [200], { event, opId: opId() })
}
