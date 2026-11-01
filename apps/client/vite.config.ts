import babel from '@rolldown/plugin-babel'
import pandacss from '@pandacss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import { execFileSync } from 'node:child_process'
import { defineConfig, type Plugin } from 'vite'

// HEALTH_DEV_BACKEND (e.g. http://127.0.0.1:8080) proxies the node's routes in `vite` and
// `vite preview`, so the app, sign-in and its redirects share one origin, as when the node
// serves the app (development and the e2e suite only).
const backend = process.env['HEALTH_DEV_BACKEND']
const proxy = backend === undefined ? undefined : Object.fromEntries(['/auth', '/api', '/fhir'].map((path) => [path, { target: backend }]))

// The build's identity (client-application.md § Serving and versions): HEALTH_BUILD_ID in
// release builds, else the commit. The bundle carries it, and `build.json` hands it to the edge
// that embeds the bundle, which advertises it in runtime.json for the version check.
const build = process.env['HEALTH_BUILD_ID'] ?? commit()

function commit(): string {
  try {
    return execFileSync('git', ['rev-parse', '--short=12', 'HEAD'], { encoding: 'utf8' }).trim()
  } catch {
    return 'dev'
  }
}

function buildJson(): Plugin {
  return {
    name: 'health-build-json',
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: 'build.json', source: `${JSON.stringify({ build })}\n` })
    },
  }
}

// One bundle serves central and the edges (runtime.json picks the platform);
// `--mode tauri` builds the desktop webview bundle.
export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    pandacss(),
    react(),
    babel({ presets: [reactCompilerPreset()] }),
    buildJson(),
  ],
  define: { 'import.meta.env.VITE_HEALTH_BUILD': JSON.stringify(build) },
  build: { sourcemap: true },
  ...(proxy === undefined ? {} : { server: { proxy }, preview: { proxy } }),
})
