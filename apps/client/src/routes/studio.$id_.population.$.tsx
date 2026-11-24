import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { PopulationEditor } from '../features/studio/population/PopulationEditor'

// A year's Population Estimates and the CSV import (#553).
export const Route = createFileRoute('/studio/$id_/population/$')({
  component: Population,
})

function Population() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/population/$', params: { id: next, _splat: _splat ?? '' } })} />
        <PopulationEditor key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
