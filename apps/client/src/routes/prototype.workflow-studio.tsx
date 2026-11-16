// PROTOTYPE (#481), throwaway: the Workflow Studio variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { WorkflowStudioPrototype } from '../features/studio/prototype/WorkflowStudioPrototype'

export const Route = createFileRoute('/prototype/workflow-studio')({
  component: WorkflowStudioPrototype,
})
