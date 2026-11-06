import { createFileRoute } from '@tanstack/react-router'

import { ProductsPage } from '../features/stock/ProductsPage'

export const Route = createFileRoute('/stock/products')({
  component: ProductsPage,
})
