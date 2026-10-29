import { QueryClient } from '@tanstack/react-query'
import { createRouter } from '@tanstack/react-router'

import type { Platform } from './platform'
import { routeTree } from './routeTree.gen'

export interface RouterContext {
  platform: Platform
  queryClient: QueryClient
}

export function makeRouter(context: RouterContext) {
  return createRouter({ routeTree, context })
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof makeRouter>
  }
}
