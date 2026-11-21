import { useQueries } from '@tanstack/react-query'

import { usePlatform } from '../../platform'
import { fileQuery, type StudioPackagePage } from './api'
import { isWorkflowLibrary } from './workflows/model'

/** The package's top-level Libraries that are Workflow Definitions: only their content says so. */
export function useWorkflowFiles(id: string, draft: StudioPackagePage): Set<string> {
  const platform = usePlatform()
  const libraries = draft.files.filter((f) => f.resourceType === 'Library' && !f.path.includes('/')).map((f) => f.path)
  const read = useQueries({ queries: libraries.map((path) => ({ ...fileQuery(platform.transport, id, path), retry: false })) })
  const out = new Set<string>()
  libraries.forEach((path, n) => {
    const content = read[n]?.data?.content
    if (content === undefined) return
    try {
      if (isWorkflowLibrary(JSON.parse(content))) out.add(path)
    } catch {
      // Not JSON: not a workflow.
    }
  })
  return out
}
