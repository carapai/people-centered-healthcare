import { createFileRoute } from '@tanstack/react-router'

import { ResultsPage } from '../features/laboratory/ResultsPage'

export const Route = createFileRoute('/results')({
  component: ResultsPage,
})
