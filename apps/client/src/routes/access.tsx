import { createFileRoute } from '@tanstack/react-router'

import { AccessPage } from '../features/access/AccessPage'

export const Route = createFileRoute('/access')({
  component: AccessPage,
})
