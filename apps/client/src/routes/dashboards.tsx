import { createFileRoute } from '@tanstack/react-router'

import { DashboardsPage } from '../features/dashboards/DashboardsPage'

export const Route = createFileRoute('/dashboards')({
  component: DashboardsPage,
})
