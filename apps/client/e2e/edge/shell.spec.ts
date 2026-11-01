/// <reference lib="dom" />
// page.evaluate runs in the browser.

import { expect, test } from '@playwright/test'

import { startEdge } from './edge'

// Serving the web app from the edge (#76; lan-security.md § App delivery to browsers).

test('a LAN browser loads the app from its edge with the internet disconnected', async ({ browser }) => {
  const edge = await startEdge()
  try {
    // No service worker, so every request reaches the network, where only the edge answers.
    const context = await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })
    const outside: string[] = []
    await context.route('**/*', (route) => {
      const url = route.request().url()
      if (new URL(url).origin === edge.origin) return route.continue()
      outside.push(url)
      return route.abort('internetdisconnected')
    })
    const page = await context.newPage()
    await page.goto('/')
    await expect(page.getByRole('status')).toContainText('Facility edge')
    // A client route deep-linked from the address bar is the same page.
    await page.goto('/stock/receive')
    await expect(page.getByRole('status')).toContainText('Facility edge')
    expect(outside).toEqual([])
    await context.close()
  } finally {
    await edge.stop()
  }
})

test('the shell loads while the edge is down, and its worker caches no API response', async ({ browser }) => {
  const edge = await startEdge()
  const context = await browser.newContext({ baseURL: edge.origin })
  const page = await context.newPage()
  try {
    await page.goto('/')
    await expect(page.getByRole('status')).toContainText('Facility edge')
    await page.waitForFunction(() => navigator.serviceWorker.controller !== null)
    // API traffic through the worker, which must pass it to the network untouched.
    const statuses = await page.evaluate(() =>
      Promise.all(['/health', '/ready', '/fhir/metadata', '/api/anything'].map((path) => fetch(path).then((r) => r.status))),
    )
    expect(statuses.slice(0, 2)).toEqual([200, 200])
  } finally {
    await edge.stop()
  }
  // The edge is gone (restarting, say): the page still loads, from the cached shell.
  await page.reload()
  await expect(page.getByRole('status')).toContainText('Facility edge')
  const cached = await page.evaluate(async () => {
    const paths: string[] = []
    for (const name of await caches.keys()) {
      for (const request of await (await caches.open(name)).keys()) paths.push(new URL(request.url).pathname)
    }
    return paths
  })
  expect(cached).toContain('/')
  expect(cached).toContain('/runtime.json')
  for (const path of cached) expect(path).toMatch(/^\/(runtime\.json|assets\/[^/]+)?$/)
  await context.close()
})
