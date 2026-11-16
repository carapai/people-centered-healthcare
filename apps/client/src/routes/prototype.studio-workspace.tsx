// PROTOTYPE (#478), throwaway: the Studio package workspace variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { WorkspacePrototype } from '../features/studio/prototype/WorkspacePrototype'

export const Route = createFileRoute('/prototype/studio-workspace')({
  component: WorkspacePrototype,
})
