import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { FileEditor } from '../features/studio/FileEditor'

export const Route = createFileRoute('/studio/$id_/files/$')({
  component: File,
})

function File() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/files/$', params: { id: next, _splat: _splat ?? '' } })} />
        <FileEditor key={id} id={id} path={_splat ?? ''} />
      </>
    </CentralOnly>
  )
}
