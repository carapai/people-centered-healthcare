import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { TerminologyBrowser } from '../features/studio/terminology/TerminologyBrowser'

export const Route = createFileRoute('/studio/$id_/terminology/$')({
  component: Terminology,
})

function Terminology() {
  const { id, _splat } = Route.useParams()
  return (
    <CentralOnly>
      <TerminologyBrowser id={id} path={_splat ?? ''} />
    </CentralOnly>
  )
}
