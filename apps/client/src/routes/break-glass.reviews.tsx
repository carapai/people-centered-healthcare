import { createFileRoute } from '@tanstack/react-router'

import { ReviewsPage } from '../features/break-glass/ReviewsPage'

export const Route = createFileRoute('/break-glass/reviews')({
  component: ReviewsPage,
})
