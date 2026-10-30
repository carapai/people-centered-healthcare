import pandacss from '@pandacss/vite'
import { playwright } from '@vitest/browser-playwright'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

// Component and accessibility tests run in Chromium, so axe sees real layout and colours.
export default defineConfig({
  plugins: [pandacss({ cwd: fileURLToPath(new URL('.', import.meta.url)) }), react()],
  test: {
    include: ['test/**/*.test.{ts,tsx}'],
    setupFiles: ['./test/setup.ts'],
    browser: {
      enabled: true,
      headless: true,
      provider: playwright(),
      instances: [{ browser: 'chromium' }],
      viewport: { width: 1366, height: 768 },
    },
  },
})
