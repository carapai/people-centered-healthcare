import { css } from '@health/design-system/css'
import { Outlet, createRootRouteWithContext } from '@tanstack/react-router'

import { StatusArea } from '../features/status/StatusArea'
import type { RouterContext } from '../router'

export const Route = createRootRouteWithContext<RouterContext>()({
  component: RootLayout,
})

function RootLayout() {
  return (
    <div className={css({ minHeight: '100vh', display: 'flex', flexDirection: 'column' })}>
      <StatusArea />
      <main className={css({ flex: '1', p: '6' })}>
        <Outlet />
      </main>
    </div>
  )
}
