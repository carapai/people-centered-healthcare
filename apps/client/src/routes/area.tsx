import { createFileRoute } from '@tanstack/react-router'

import { AreaPage } from '../features/area/AreaPage'

export const Route = createFileRoute('/area')({
  component: AreaPage,
})
