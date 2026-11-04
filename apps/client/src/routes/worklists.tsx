import { createFileRoute } from '@tanstack/react-router'

import { WorklistsPage } from '../features/worklists/WorklistsPage'

export const Route = createFileRoute('/worklists')({
  component: WorklistsPage,
})
