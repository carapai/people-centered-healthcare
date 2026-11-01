import { createFileRoute } from '@tanstack/react-router'

import { SupervisePage } from '../features/till/SupervisePage'

export const Route = createFileRoute('/till/supervise')({
  component: SupervisePage,
})
