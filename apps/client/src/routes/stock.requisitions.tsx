import { createFileRoute } from '@tanstack/react-router'

import { RequisitionsPage } from '../features/stock/RequisitionsPage'

export const Route = createFileRoute('/stock/requisitions')({
  component: RequisitionsPage,
})
