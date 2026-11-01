import { expect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { startEdge } from './edge'

// The edge admin status page (#77; facility-edge.md § Health monitoring), served by the edge to a
// browser on the node itself.

test('the edge shows its own status and alerts, reached from the home page', async ({ browser }) => {
  const edge = await startEdge()
  try {
    const page = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await page.goto('/')
    await page.getByRole('link', { name: 'Edge status' }).click()
    await expect(page.getByRole('heading', { name: 'Edge status' })).toBeVisible()
    // This edge has no backup directory, so it says so.
    await expect(page.getByRole('alert')).toContainText('Backups are not configured on this node.')
    await expect(page.getByRole('region', { name: 'Node' })).toContainText('facility-edge')
    await expect(page.getByRole('region', { name: 'Storage' })).toContainText('Data disk')
    await expect(page.getByText(/Not reported yet: sync/)).toBeVisible()
    await noViolations(page)
  } finally {
    await edge.stop()
  }
})
