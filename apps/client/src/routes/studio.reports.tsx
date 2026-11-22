import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { DesignSection } from '../features/studio/DesignSection'

// A section of the Studio (#855); `?new=true` opens its New panel.
export const Route = createFileRoute('/studio/reports')({
  validateSearch: (search: Record<string, unknown>): { new?: boolean } => (search['new'] === true || search['new'] === 'true' ? { new: true } : {}),
  component: Section,
})

function Section() {
  const search = Route.useSearch()
  return (
    <CentralOnly>
      <DesignSection key={String(search.new === true)} section="reports" startNew={search.new === true} />
    </CentralOnly>
  )
}
