import { defineConfig } from '@pandacss/dev'

export default defineConfig({
  // Panda 2 emits invalid, token-less CSS without these, and `panda doctor` still passes (ADR 0054).
  presets: ['@pandacss/preset-base', '@pandacss/preset-panda'],
  preflight: true,
  include: ['./src/**/*.{ts,tsx}'],
  exclude: [],
  outdir: 'styled-system',
  jsxFramework: 'react',
})
