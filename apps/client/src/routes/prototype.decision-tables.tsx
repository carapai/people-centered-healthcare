// PROTOTYPE (#513), throwaway: the decision-table editor variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { DecisionTablePrototype } from '../features/studio/prototype/DecisionTablePrototype'

export const Route = createFileRoute('/prototype/decision-tables')({
  component: DecisionTablePrototype,
})
