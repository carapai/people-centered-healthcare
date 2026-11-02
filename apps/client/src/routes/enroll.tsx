import { createFileRoute } from '@tanstack/react-router'

import { EnrollPage } from '../features/enrollment/EnrollPage'

export const Route = createFileRoute('/enroll')({
  component: EnrollPage,
})
