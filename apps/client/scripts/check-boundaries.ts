// Boundary checks that oxlint cannot express (client-application.md, ADR 0037):
// - feature and route code never checks which runtime it runs in;
// - browser storage APIs appear only in the preferences allowlist.
// Usage: bun scripts/check-boundaries.ts [srcDir]

import { readdir, readFile } from 'node:fs/promises'
import { join, relative } from 'node:path'

const srcDir = process.argv[2] ?? 'src'

const runtimeChecks: [RegExp, string][] = [
  [/__TAURI/, 'Tauri globals'],
  [/import\.meta\.env\.MODE/, 'the Vite build mode'],
  [/\bisTauri\b/, 'isTauri'],
  [/platform\.mode\b|\.mode\s*===\s*['"](central|edge|tauri)['"]/, 'the runtime mode'],
]

const storageApis: [RegExp, string][] = [
  [/\blocalStorage\b/, 'localStorage'],
  [/\bsessionStorage\b/, 'sessionStorage'],
  [/\bindexedDB\b/, 'IndexedDB'],
  [/navigator\.storage\b|getDirectory\s*\(/, 'OPFS'],
  [/\bcaches\.open\b/, 'the Cache API'],
]

/**
 * Files allowed to use browser storage: user-keyed UI preferences, and the OAuth transaction
 * (PKCE verifier and state) kept across the sign-in redirect, which is removed at the callback.
 * Neither is clinical data.
 */
const storageAllowlist = new Set(['platform/preferences.ts', 'platform/sign-in.ts'])

/** Code allowed to know the runtime: the composition boundary and the status area. */
function mayCheckRuntime(file: string): boolean {
  return file.startsWith('platform/') || file === 'features/status/StatusArea.tsx'
}

async function* sourceFiles(dir: string): AsyncGenerator<string> {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) yield* sourceFiles(path)
    else if (/\.(ts|tsx)$/.test(entry.name) && !entry.name.endsWith('.gen.ts')) yield path
  }
}

const problems: string[] = []
for await (const path of sourceFiles(srcDir)) {
  const file = relative(srcDir, path).split('\\').join('/')
  const lines = (await readFile(path, 'utf8')).split('\n')
  lines.forEach((line, index) => {
    const at = `${join(srcDir, file)}:${index + 1}`
    if (!mayCheckRuntime(file)) {
      for (const [pattern, what] of runtimeChecks) {
        if (pattern.test(line)) problems.push(`${at}: feature code must not check ${what}; use Platform.capabilities`)
      }
    }
    if (!storageAllowlist.has(file)) {
      for (const [pattern, what] of storageApis) {
        if (pattern.test(line)) problems.push(`${at}: ${what} is not allowed here; browsers never persist clinical data (ADR 0037)`)
      }
    }
  })
}

for (const problem of problems) console.error(problem)
if (problems.length > 0) {
  console.error(`check-boundaries: ${problems.length} problem(s)`)
  process.exit(1)
}
console.log('check-boundaries: ok')
