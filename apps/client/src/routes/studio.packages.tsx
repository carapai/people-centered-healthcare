import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { PackagesPage } from '../features/studio/PackagesPage'

export const Route = createFileRoute('/studio/packages')({
  component: () => (
    <CentralOnly>
      <PackagesPage />
    </CentralOnly>
  ),
})
