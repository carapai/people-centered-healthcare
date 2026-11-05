import { createFileRoute } from '@tanstack/react-router'

import { ConsultationPage } from '../features/consultation/ConsultationPage'

export const Route = createFileRoute('/consultation')({
  component: ConsultationPage,
})
