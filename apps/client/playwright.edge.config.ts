import { defineConfig, devices } from '@playwright/test'

// The edge suite (#76): the app as a facility-edge binary serves it, embedded. Each test starts
// its own edge (e2e/edge/edge.ts); build the client before the binary so the bundle is embedded.
export default defineConfig({
  testDir: 'e2e/edge',
  fullyParallel: false,
  workers: 1,
  // Each test starts a fresh edge, whose first start runs every migration: SurrealDB 4 syncs on
  // each schema statement, which takes over a minute on macOS (ADR 0060).
  timeout: 420_000,
  // The edge is a debug build, whose SurrealDB runs unoptimised: a write that indexes a resource
  // for search takes seconds (a first registration about 4 s on an idle Mac, #907), and more on a
  // loaded machine, so Playwright's 5 s default for an assertion fails a working edge.
  expect: { timeout: 30_000 },
  retries: process.env['CI'] === undefined ? 0 : 1,
  reporter: process.env['CI'] === undefined ? 'list' : [['list'], ['html', { open: 'never', outputFolder: 'playwright-report/edge' }]],
  use: { trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
})
