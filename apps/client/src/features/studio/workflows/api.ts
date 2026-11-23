import type { StudioDecisionCheck, StudioDecisionTry, StudioProblem, StudioSample, StudioSandbox, StudioSandboxMove, StudioWorkflowAction } from '@health/api-client'

import type { Transport } from '../../../platform/types'
import { StudioError } from '../api'
import type { Value, WorkflowDefinition } from './model'

// The release's typed-action catalogue (ADR 0043; #498), read from `/api/studio/workflow-actions`
// at central: each action's reference, permission, compensation and parameter schema. It is the
// catalogue publication checks Workflow Definitions against.

/** The release's typed actions. They change only with the release, so they are read once. */
export function workflowActionsQuery(transport: Transport) {
  return {
    queryKey: ['studio', 'workflow-actions'] as const,
    staleTime: Number.POSITIVE_INFINITY,
    queryFn: async ({ signal }: { signal?: AbortSignal } = {}): Promise<StudioWorkflowAction[]> => {
      const response = await transport.request({
        method: 'GET',
        path: '/studio/workflow-actions',
        ...(signal === undefined ? {} : { signal }),
      })
      const { parseResponse } = await import('@health/api-client')
      const value = parseResponse('listStudioWorkflowActions', response.status, response.body)
      if (response.status === 200) return (value as { actions: StudioWorkflowAction[] }).actions
      const problem = value as StudioProblem | null
      throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
    },
  }
}

// The decision-table editor (#516): central evaluates the draft table, unsaved edits included,
// on the FHIRPath engine submission's simulation uses.

export type { StudioDecisionCheck, StudioDecisionTry, StudioSample } from '@health/api-client'

async function post<T>(transport: Transport, operation: 'tryStudioDecisionTable' | 'checkStudioDecisionTable', id: string, kind: 'try' | 'check', body: unknown, signal?: AbortSignal): Promise<T> {
  const response = await transport.request({
    method: 'POST',
    path: `/studio/packages/${encodeURIComponent(id)}/decision-tables/${kind}`,
    body,
    ...(signal === undefined ? {} : { signal }),
  })
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse(operation, response.status, response.body)
  if (response.status === 200) return value as T
  const problem = value as StudioProblem | null
  throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
}

/** *Try a patient*: the outcome of draft `table` for `inputs`, the rule that fired and how each
 * of `nodes` (parts of a rule's tree, as FHIRPath) came out. */
export function tryTable(
  transport: Transport,
  id: string,
  body: {
    definition: WorkflowDefinition
    table: string
    inputs: Record<string, StudioSample>
    nodes: string[]
  },
  signal?: AbortSignal,
) {
  return post<StudioDecisionTry>(transport, 'tryStudioDecisionTable', id, 'try', body, signal)
}

/** The editor's checks on draft `table`: gaps, rules that never fire, and the saved test cases. */
export function checkTable(
  transport: Transport,
  id: string,
  body: {
    definition: WorkflowDefinition
    table: string
    samples: Record<string, StudioSample[]>
  },
  signal?: AbortSignal,
) {
  return post<StudioDecisionCheck>(transport, 'checkStudioDecisionTable', id, 'check', body, signal)
}

// The Workflow Studio's sandbox (#499): central plays the draft, unsaved edits included, in the
// engine's simulation with every action stubbed, and answers each step. The browser keeps only
// the author's answers and the moves; it never decides a transition.

export type { StudioSandbox, StudioSandboxMove, StudioSandboxStep } from '@health/api-client'

/** Plays `run` on draft `definition` from the start: what held after each move taken, the
 * guards (and decision tables) the next move needs answered, and the run as a `workflow-scenario` file. */
export async function playSandbox(
  transport: Transport,
  id: string,
  body: {
    definition: WorkflowDefinition
    guards: Record<string, boolean>
    tables: Record<string, Value>
    gates: Record<string, string>
    moves: StudioSandboxMove[]
    name?: string
  },
  signal?: AbortSignal,
): Promise<StudioSandbox> {
  const response = await transport.request({
    method: 'POST',
    path: `/studio/packages/${encodeURIComponent(id)}/workflows/sandbox`,
    body,
    ...(signal === undefined ? {} : { signal }),
  })
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse('playStudioWorkflowSandbox', response.status, response.body)
  if (response.status === 200) return value as StudioSandbox
  const problem = value as StudioProblem | null
  throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
}
