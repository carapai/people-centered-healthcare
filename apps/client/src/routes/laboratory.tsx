import { createFileRoute } from '@tanstack/react-router'

import { LaboratoryPage } from '../features/laboratory/LaboratoryPage'

export const Route = createFileRoute('/laboratory')({
  component: LaboratoryPage,
})
