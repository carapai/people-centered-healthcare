import { createFileRoute } from '@tanstack/react-router'

import { RolesPage } from '../features/places/RolesPage'

export const Route = createFileRoute('/places/roles')({
  component: RolesPage,
})
