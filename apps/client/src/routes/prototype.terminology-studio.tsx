// PROTOTYPE (#512), throwaway: the terminology screen variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { TerminologyPrototype } from '../features/studio/prototype/TerminologyPrototype'

export const Route = createFileRoute('/prototype/terminology-studio')({
  component: TerminologyPrototype,
})
