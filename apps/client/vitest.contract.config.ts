import { defineConfig } from 'vitest/config'

// The FHIR contract suite, against the Rust FhirServer (test/contract/server.ts starts it).
export default defineConfig({
  test: {
    include: ['test/contract/**/*.test.ts'],
    globalSetup: ['test/contract/server.ts'],
    testTimeout: 15_000,
  },
})
