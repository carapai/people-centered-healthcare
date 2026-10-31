import { createFileRoute } from '@tanstack/react-router'

import { RegistrationPage } from '../features/registration/RegistrationPage'

export const Route = createFileRoute('/registration')({
  component: RegistrationPage,
})
