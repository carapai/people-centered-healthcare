import { createFileRoute } from '@tanstack/react-router'

import { AlertsPage } from '../features/break-glass/AlertsPage'

export const Route = createFileRoute('/break-glass/alerts')({
  component: AlertsPage,
})
