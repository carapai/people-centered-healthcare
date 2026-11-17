// PROTOTYPE (#563), throwaway: the health worker workspace layout variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { WorkspacePrototype } from '../features/worklists/prototype/WorkspacePrototype'

export const Route = createFileRoute('/prototype/workspace')({
  component: WorkspacePrototype,
})
