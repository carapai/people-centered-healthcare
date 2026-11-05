import { createFileRoute } from '@tanstack/react-router'

import { QueuePage } from '../features/reception/QueuePage'

export const Route = createFileRoute('/reception/queues')({
  component: QueuePage,
})
