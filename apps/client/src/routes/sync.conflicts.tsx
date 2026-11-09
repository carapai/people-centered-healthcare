import { createFileRoute } from '@tanstack/react-router'

import { ConflictsPage } from '../features/conflicts/ConflictsPage'

export const Route = createFileRoute('/sync/conflicts')({
  component: ConflictsPage,
})
