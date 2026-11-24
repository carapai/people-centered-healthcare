import { defineConfig, devices } from '@playwright/test'

// The end-to-end suite (testing.md: Playwright on the web, nightly; #228): the built client in
// `vite preview`, proxying /auth, /api and /fhir to a real central-server over SurrealDB.
// The ports move with the backend's variables (e2e/backend.ts), e.g. to run beside a dev backend:
// HEALTH_E2E_CENTRAL_PORT, HEALTH_E2E_DB_PORT, HEALTH_E2E_READY_PORT and HEALTH_E2E_ORIGIN (the
// app's, whose port `vite preview` takes); HEALTH_E2E_OUT_DIR builds the app somewhere other than
// dist.
const origin = process.env['HEALTH_E2E_ORIGIN'] ?? 'http://localhost:4173'
const previewPort = new URL(origin).port || '4173'
const central = `http://127.0.0.1:${process.env['HEALTH_E2E_CENTRAL_PORT'] ?? '18080'}`
const ready = `http://127.0.0.1:${process.env['HEALTH_E2E_READY_PORT'] ?? '18081'}/`
const outDir = process.env['HEALTH_E2E_OUT_DIR'] ?? 'dist'

export default defineConfig({
  testDir: 'e2e',
  // The edge suite has its own config (playwright.edge.config.ts).
  testIgnore: 'edge/**',
  fullyParallel: false,
  workers: 1,
  retries: process.env['CI'] === undefined ? 0 : 1,
  reporter: process.env['CI'] === undefined ? 'list' : [['list'], ['html', { open: 'never' }]],
  use: { baseURL: origin, trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: [
    // The backend's readiness port opens only after it has published and seeded everything, so no
    // spec starts on a half-seeded backend (#893); central's own /ready comes before seeding.
    { command: 'bun e2e/backend.ts', url: ready, timeout: 180_000, reuseExistingServer: false },
    {
      command: `bunx --bun vite build --outDir ${outDir} && bunx --bun vite preview --outDir ${outDir} --port ${previewPort} --strictPort`,
      url: origin,
      env: { HEALTH_DEV_BACKEND: central },
      timeout: 180_000,
      reuseExistingServer: false,
    },
  ],
})
