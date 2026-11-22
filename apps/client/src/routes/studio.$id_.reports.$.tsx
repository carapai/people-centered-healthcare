import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { ReportDesigner } from '../features/studio/reports/ReportDesigner'

// A Report (#559).
export const Route = createFileRoute('/studio/$id_/reports/$')({
  component: Reports,
})

function Reports() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/reports/$', params: { id: next, _splat: _splat ?? '' } })} />
        <ReportDesigner key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
