import babel from '@rolldown/plugin-babel'
import pandacss from '@pandacss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// One bundle serves central and the edges (runtime.json picks the platform);
// `--mode tauri` builds the desktop webview bundle.
export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    pandacss(),
    react(),
    babel({ presets: [reactCompilerPreset()] }),
  ],
  build: { sourcemap: true },
})
