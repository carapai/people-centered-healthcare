// PROTOTYPE (#519), throwaway: the local CodeSystem editor variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { CodeSystemEditorPrototype } from '../features/studio/prototype/CodeSystemEditorPrototype'

export const Route = createFileRoute('/prototype/codesystem-editor')({
  component: CodeSystemEditorPrototype,
})
