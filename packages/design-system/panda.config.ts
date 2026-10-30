import { defineConfig } from '@pandacss/dev'

import { conditions } from './src/theme/conditions'
import { slotRecipes } from './src/theme/recipes'
import { semanticTokens } from './src/theme/semantic-tokens'

// The @health/design-system preset (client-application.md § Design system). `panda lib` writes it
// to dist/panda/; apps load it with `designSystem: '@health/design-system'`.
export default defineConfig({
  // Panda 2 emits invalid, token-less CSS without these, and `panda doctor` still passes (ADR 0054).
  presets: ['@pandacss/preset-base', '@pandacss/preset-panda'],
  preflight: true,
  include: ['./src/**/*.{ts,tsx}', './example/**/*.{ts,tsx}'],
  exclude: [],
  outdir: 'styled-system',
  jsxFramework: 'react',
  conditions: { extend: conditions },
  theme: { extend: { semanticTokens, slotRecipes } },
  globalCss: {
    html: { colorScheme: 'light', '&[data-theme=dark]': { colorScheme: 'dark' } },
    body: { bg: 'bg.canvas', color: 'fg.default' },
  },
})
