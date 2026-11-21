import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { AgeBandSetEditor } from '../features/studio/indicators/AgeBandSetEditor'

// An Age Band Set's table (#551).
export const Route = createFileRoute('/studio/$id_/age-bands/$')({
  component: AgeBands,
})

function AgeBands() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/age-bands/$', params: { id: next, _splat: _splat ?? '' } })} />
        <AgeBandSetEditor key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
