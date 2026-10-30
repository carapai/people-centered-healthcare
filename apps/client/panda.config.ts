import { defineConfig } from '@pandacss/dev'

// Tokens, conditions, recipes and presets come from @health/design-system (built with
// `panda lib`); the app's own styles import `css` from '@health/design-system/css'.
export default defineConfig({
  designSystem: '@health/design-system',
  preflight: true,
  include: ['./src/**/*.{ts,tsx}'],
  exclude: [],
  outdir: 'styled-system',
  jsxFramework: 'react',
})
