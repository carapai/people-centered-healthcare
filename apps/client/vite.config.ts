import babel from '@rolldown/plugin-babel'
import pandacss from '@pandacss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// HEALTH_DEV_BACKEND (e.g. http://127.0.0.1:8080) proxies the node's routes in `vite` and
// `vite preview`, so the app, sign-in and its redirects share one origin, as when the node
// serves the app (development and the e2e suite only).
const backend = process.env['HEALTH_DEV_BACKEND']
const proxy = backend === undefined ? undefined : Object.fromEntries(['/auth', '/api', '/fhir'].map((path) => [path, { target: backend }]))

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
  ...(proxy === undefined ? {} : { server: { proxy }, preview: { proxy } }),
})
