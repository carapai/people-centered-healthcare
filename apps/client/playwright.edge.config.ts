import { defineConfig, devices } from '@playwright/test'

// The edge suite (#76): the app as a facility-edge binary serves it, embedded. Each test starts
// its own edge (e2e/edge/edge.ts); build the client before the binary so the bundle is embedded.
export default defineConfig({
  testDir: 'e2e/edge',
  fullyParallel: false,
  workers: 1,
  retries: process.env['CI'] === undefined ? 0 : 1,
  reporter: process.env['CI'] === undefined ? 'list' : [['list'], ['html', { open: 'never', outputFolder: 'playwright-report/edge' }]],
  use: { trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
})
