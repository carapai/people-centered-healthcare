// PROTOTYPE (#534), throwaway: the indicator editor layout variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { IndicatorEditorPrototype } from '../features/studio/prototype/IndicatorEditorPrototype'

export const Route = createFileRoute('/prototype/indicator-editor')({
  component: IndicatorEditorPrototype,
})
