import { createFileRoute } from '@tanstack/react-router'

import { ExplorerPage } from '../features/explorer/ExplorerPage'

export const Route = createFileRoute('/explore')({
  component: ExplorerPage,
})
