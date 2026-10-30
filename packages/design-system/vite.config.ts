import pandacss from '@pandacss/vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

// The example page (example/): every recipe in every theme and density.
export default defineConfig({
  root: 'example',
  // Panda runs from the package root, where panda.config.ts lives.
  plugins: [pandacss({ cwd: fileURLToPath(new URL('.', import.meta.url)) }), react()],
  build: { outDir: '../dist/example', emptyOutDir: true },
})
