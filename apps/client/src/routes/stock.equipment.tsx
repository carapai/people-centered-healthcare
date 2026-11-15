import { createFileRoute } from '@tanstack/react-router'

import { EquipmentPage } from '../features/stock/EquipmentPage'

export const Route = createFileRoute('/stock/equipment')({
  component: EquipmentPage,
})
