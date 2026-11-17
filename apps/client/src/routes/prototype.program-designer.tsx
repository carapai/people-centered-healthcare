// PROTOTYPE (#482), throwaway: the Program Designer variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { ProgramDesignerPrototype } from '../features/studio/prototype/ProgramDesignerPrototype'

export const Route = createFileRoute('/prototype/program-designer')({
  component: ProgramDesignerPrototype,
})
