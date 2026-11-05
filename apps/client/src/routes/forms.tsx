import { createFileRoute } from '@tanstack/react-router'

import { FormsPage } from '../features/forms/FormsPage'

export const Route = createFileRoute('/forms')({
  component: FormsPage,
})
