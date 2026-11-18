// PROTOTYPE (#647), throwaway: three visual directions for the design system. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { VisualDirectionPrototype } from '../features/design-system-prototype/VisualDirectionPrototype'

export const Route = createFileRoute('/prototype/visual-direction')({
  validateSearch: (s: Record<string, unknown>): { variant?: string; theme?: string; density?: string } => ({
    ...(typeof s['variant'] === 'string' ? { variant: s['variant'] } : {}),
    ...(typeof s['theme'] === 'string' ? { theme: s['theme'] } : {}),
    ...(typeof s['density'] === 'string' ? { density: s['density'] } : {}),
  }),
  component: VisualDirectionPrototype,
})
