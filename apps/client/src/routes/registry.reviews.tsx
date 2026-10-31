import { createFileRoute } from '@tanstack/react-router'

import { ReviewQueuePage } from '../features/registry-review/ReviewQueuePage'

export const Route = createFileRoute('/registry/reviews')({
  component: ReviewQueuePage,
})
