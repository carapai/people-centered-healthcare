import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { DashboardDesigner } from '../features/studio/dashboards/DashboardDesigner'

// A standard Dashboard (#879).
export const Route = createFileRoute('/studio/$id_/dashboards/$')({
  component: Dashboards,
})

function Dashboards() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/dashboards/$', params: { id: next, _splat: _splat ?? '' } })} />
        <DashboardDesigner key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
