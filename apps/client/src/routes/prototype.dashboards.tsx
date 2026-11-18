// PROTOTYPE (#597), throwaway: the data explorer and Dashboard variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { DashboardsPrototype } from '../features/analytics/prototype/DashboardsPrototype'

export const Route = createFileRoute('/prototype/dashboards')({
  component: DashboardsPrototype,
})
