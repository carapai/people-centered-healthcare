import { createFileRoute } from '@tanstack/react-router'

import { ReportingPage } from '../features/places/ReportingPage'

export const Route = createFileRoute('/places/reporting')({
  component: ReportingPage,
})
