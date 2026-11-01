import { createFileRoute } from '@tanstack/react-router'

import { TillPage } from '../features/till/TillPage'

export const Route = createFileRoute('/till/')({
  component: TillPage,
})
