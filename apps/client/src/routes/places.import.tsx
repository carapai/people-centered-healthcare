import { createFileRoute } from '@tanstack/react-router'

import { ImportPage } from '../features/places/ImportPage'

export const Route = createFileRoute('/places/import')({
  component: ImportPage,
})
