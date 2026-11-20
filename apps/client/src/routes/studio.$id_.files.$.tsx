import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { FileEditor } from '../features/studio/FileEditor'

export const Route = createFileRoute('/studio/$id_/files/$')({
  component: File,
})

function File() {
  const { id, _splat } = Route.useParams()
  return (
    <CentralOnly>
      <FileEditor id={id} path={_splat ?? ''} />
    </CentralOnly>
  )
}
