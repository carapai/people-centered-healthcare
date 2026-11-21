import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { ProgramDesigner } from '../features/studio/programs/ProgramDesigner'

export const Route = createFileRoute('/studio/$id_/programs/$')({
  component: Program,
})

function Program() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/programs/$', params: { id: next, _splat: _splat ?? '' } })} />
        <ProgramDesigner key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
