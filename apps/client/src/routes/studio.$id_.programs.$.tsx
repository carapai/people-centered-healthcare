import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { ProgramDesigner } from '../features/studio/programs/ProgramDesigner'

export const Route = createFileRoute('/studio/$id_/programs/$')({
  component: Program,
})

function Program() {
  const { id, _splat } = Route.useParams()
  return (
    <CentralOnly>
      <ProgramDesigner id={id} path={_splat ?? ''} />
    </CentralOnly>
  )
}
