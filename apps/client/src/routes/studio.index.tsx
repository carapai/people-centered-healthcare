import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { StudioHome } from '../features/studio/StudioHome'

// The Studio starts from what is designed (#855); the packages are one of its sections.
export const Route = createFileRoute('/studio/')({
  component: () => (
    <CentralOnly>
      <StudioHome />
    </CentralOnly>
  ),
})
