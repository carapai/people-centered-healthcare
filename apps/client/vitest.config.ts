import { defineConfig } from 'vitest/config'

// Unit tests of the platform layer, in Bun; the contract suite has its own config.
export default defineConfig({ test: { include: ['test/**/*.test.ts'], exclude: ['test/contract/**'] } })
