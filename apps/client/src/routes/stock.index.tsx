import { createFileRoute } from '@tanstack/react-router'

import { StockPage } from '../features/stock/StockPage'

export const Route = createFileRoute('/stock/')({
  component: StockPage,
})
