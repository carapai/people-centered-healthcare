import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { FormDesigner } from '../features/studio/forms/FormDesigner'

export const Route = createFileRoute('/studio/$id_/forms/$')({
  component: Form,
})

function Form() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/forms/$', params: { id: next, _splat: _splat ?? '' } })} />
        <FormDesigner key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
