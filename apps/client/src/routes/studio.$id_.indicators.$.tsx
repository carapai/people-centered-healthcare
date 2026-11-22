import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { IndicatorEditor } from '../features/studio/indicators/IndicatorEditor'

// An indicator (#537).
export const Route = createFileRoute('/studio/$id_/indicators/$')({
  component: Indicators,
})

function Indicators() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/indicators/$', params: { id: next, _splat: _splat ?? '' } })} />
        <IndicatorEditor key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
