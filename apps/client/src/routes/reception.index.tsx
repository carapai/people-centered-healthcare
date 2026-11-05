import { createFileRoute } from '@tanstack/react-router'

import { ReceptionPage } from '../features/reception/ReceptionPage'

export const Route = createFileRoute('/reception/')({
  component: ReceptionPage,
})
