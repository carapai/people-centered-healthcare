import { defineConfig, devices } from '@playwright/test'

// The end-to-end suite (testing.md: Playwright on the web, nightly; #228): the built client in
// `vite preview`, proxying /auth, /api and /fhir to a real central-server over SurrealDB.
export default defineConfig({
  testDir: 'e2e',
  fullyParallel: false,
  workers: 1,
  retries: process.env['CI'] === undefined ? 0 : 1,
  reporter: process.env['CI'] === undefined ? 'list' : [['list'], ['html', { open: 'never' }]],
  use: { baseURL: 'http://localhost:4173', trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: [
    { command: 'bun e2e/backend.ts', url: 'http://127.0.0.1:18080/ready', timeout: 180_000, reuseExistingServer: false },
    {
      command: 'bun run build && bunx --bun vite preview --port 4173 --strictPort',
      url: 'http://localhost:4173',
      env: { HEALTH_DEV_BACKEND: 'http://127.0.0.1:18080' },
      timeout: 180_000,
      reuseExistingServer: false,
    },
  ],
})
