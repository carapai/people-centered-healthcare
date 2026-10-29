import { QueryClient } from '@tanstack/react-query'

// Clinical data lives only in this in-memory cache: 5-minute gcTime, never persisted (ADR 0037).
export function makeQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { gcTime: 5 * 60 * 1000, retry: 1 },
      mutations: { retry: false },
    },
  })
}
