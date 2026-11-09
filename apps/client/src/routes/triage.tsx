import { createFileRoute } from '@tanstack/react-router'

import { TriagePage } from '../features/triage/TriagePage'

export const Route = createFileRoute('/triage')({
  component: TriagePage,
})
