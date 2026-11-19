import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { PackagePage } from '../features/studio/PackagePage'

export const Route = createFileRoute('/studio/$id')({
  component: Package,
})

function Package() {
  const { id } = Route.useParams()
  return (
    <CentralOnly>
      <PackagePage id={id} />
    </CentralOnly>
  )
}
