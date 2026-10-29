import './index.css'

import { QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { PlatformProvider, composePlatform } from './platform'
import { makeQueryClient } from './queryClient'
import { makeRouter } from './router'

const root = document.getElementById('root')
if (root === null) {
  throw new Error('index.html must contain #root')
}

const platform = await composePlatform()
const queryClient = makeQueryClient()
const router = makeRouter({ platform, queryClient })

createRoot(root).render(
  <StrictMode>
    <PlatformProvider platform={platform}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </PlatformProvider>
  </StrictMode>,
)
