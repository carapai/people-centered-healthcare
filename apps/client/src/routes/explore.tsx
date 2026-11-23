import { createFileRoute } from '@tanstack/react-router'

import { readExplorerSearch } from '../features/dashboards/api'
import { ExplorerPage } from '../features/explorer/ExplorerPage'

export const Route = createFileRoute('/explore')({
  // `v` is a saved Visualization opened from a Dashboard (*Open in explorer*, #602).
  validateSearch: (search: Record<string, unknown>): { v?: string } => {
    const v = search['v']
    if (typeof v === 'string' && v !== '') return { v }
    // The router may hand back the JSON already parsed.
    return typeof v === 'object' && v !== null ? { v: JSON.stringify(v) } : {}
  },
  component: Explore,
})

function Explore() {
  const { v } = Route.useSearch()
  return <ExplorerPage key={v ?? ''} initial={readExplorerSearch(v)} />
}
