// PROTOTYPE (#479), throwaway: the Studio form designer variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { FormDesignerPrototype } from '../features/studio/prototype/FormDesignerPrototype'

export const Route = createFileRoute('/prototype/form-designer')({
  component: FormDesignerPrototype,
})
