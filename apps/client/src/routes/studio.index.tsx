import { createFileRoute } from '@tanstack/react-router'

import { PackagesPage } from '../features/studio/PackagesPage'
import { CentralOnly } from '../features/studio/CentralOnly'

export const Route = createFileRoute('/studio/')({
  component: () => (
    <CentralOnly>
      <PackagesPage />
    </CentralOnly>
  ),
})
