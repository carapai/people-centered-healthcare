import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { TerminologyBrowser } from '../features/studio/terminology/TerminologyBrowser'

export const Route = createFileRoute('/studio/$id_/terminology/$')({
  component: Terminology,
})

function Terminology() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/terminology/$', params: { id: next, _splat: _splat ?? '' } })} />
        <TerminologyBrowser key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
