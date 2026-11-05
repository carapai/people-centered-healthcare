import { createFileRoute } from '@tanstack/react-router'

import { RecordPage } from '../features/records/RecordPage'

export const Route = createFileRoute('/records/$type/$id')({
  component: Record,
})

function Record() {
  const { type, id } = Route.useParams()
  return <RecordPage type={type} id={id} />
}
