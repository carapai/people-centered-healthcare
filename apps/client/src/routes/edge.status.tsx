import { createFileRoute } from '@tanstack/react-router'

import { StatusPage } from '../features/edge-status/StatusPage'

export const Route = createFileRoute('/edge/status')({
  component: StatusPage,
})
