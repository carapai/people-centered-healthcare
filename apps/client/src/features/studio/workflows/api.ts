import type { StudioProblem, StudioWorkflowAction } from '@health/api-client'

import type { Transport } from '../../../platform/types'
import { StudioError } from '../api'

// The release's typed-action catalogue (ADR 0043; #498), read from `/api/studio/workflow-actions`
// at central: each action's reference, permission, compensation and parameter schema. It is the
// catalogue publication checks Workflow Definitions against.

/** The release's typed actions. They change only with the release, so they are read once. */
export function workflowActionsQuery(transport: Transport) {
  return {
    queryKey: ['studio', 'workflow-actions'] as const,
    staleTime: Number.POSITIVE_INFINITY,
    queryFn: async ({ signal }: { signal?: AbortSignal } = {}): Promise<StudioWorkflowAction[]> => {
      const response = await transport.request({ method: 'GET', path: '/studio/workflow-actions', ...(signal === undefined ? {} : { signal }) })
      const { parseResponse } = await import('@health/api-client')
      const value = parseResponse('listStudioWorkflowActions', response.status, response.body)
      if (response.status === 200) return (value as { actions: StudioWorkflowAction[] }).actions
      const problem = value as StudioProblem | null
      throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
    },
  }
}
