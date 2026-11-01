import { createFileRoute } from '@tanstack/react-router'

import { ReceivePage } from '../features/stock/ReceivePage'

export const Route = createFileRoute('/stock/receive')({
  component: ReceivePage,
})
