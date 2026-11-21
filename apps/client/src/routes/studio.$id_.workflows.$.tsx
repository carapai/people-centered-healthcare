import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { CentralOnly } from '../features/studio/CentralOnly'
import { EditorBar } from '../features/studio/EditorBar'
import { WorkflowStudio } from '../features/studio/workflows/WorkflowStudio'

// Split into its own chunk (autoCodeSplitting), so @xyflow/react and dagre load only here.
export const Route = createFileRoute('/studio/$id_/workflows/$')({
  // `?table=` opens one of its decision tables (#855).
  validateSearch: (search: Record<string, unknown>): { table?: string } => (typeof search['table'] === 'string' && search['table'] !== '' ? { table: search['table'] } : {}),
  component: Workflow,
})

function Workflow() {
  const { id, _splat } = Route.useParams()
  const navigate = useNavigate()
  const { table } = Route.useSearch()
  return (
    <CentralOnly>
      <>
        <EditorBar id={id} onOpen={(next) => void navigate({ to: '/studio/$id/workflows/$', params: { id: next, _splat: _splat ?? '' }, search: table === undefined ? {} : { table } })} />
        <WorkflowStudio key={`${id}#${table ?? ''}`} id={id} path={_splat ?? ''} {...(table === undefined ? {} : { table })} />
      </>
    </CentralOnly>
  )
}
