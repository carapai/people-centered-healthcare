import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { FormDesigner } from '../features/studio/forms/FormDesigner'

export const Route = createFileRoute('/studio/$id_/forms/$')({
  component: Form,
})

function Form() {
  const { id, _splat } = Route.useParams()
  return (
    <CentralOnly>
      <FormDesigner id={id} path={_splat ?? ''} />
    </CentralOnly>
  )
}
