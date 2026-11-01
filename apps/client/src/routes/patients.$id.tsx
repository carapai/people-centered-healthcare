import { createFileRoute } from '@tanstack/react-router'

import { PatientPage } from '../features/patient/PatientPage'

export const Route = createFileRoute('/patients/$id')({
  component: Patient,
})

function Patient() {
  const { id } = Route.useParams()
  return <PatientPage id={id} />
}
