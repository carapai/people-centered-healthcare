import { createFileRoute } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { WorkflowStudio } from '../features/studio/workflows/WorkflowStudio'

// Split into its own chunk (autoCodeSplitting), so @xyflow/react and dagre load only here.
export const Route = createFileRoute('/studio/$id_/workflows/$')({
  component: Workflow,
})

function Workflow() {
  const { id, _splat } = Route.useParams()
  return (
    <CentralOnly>
      <WorkflowStudio id={id} path={_splat ?? ''} />
    </CentralOnly>
  )
}
