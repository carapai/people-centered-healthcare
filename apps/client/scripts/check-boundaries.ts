// Boundary checks that oxlint cannot express (client-application.md, ADR 0037):
// - feature and route code never checks which runtime it runs in;
// - browser storage APIs appear only in the preferences allowlist;
// - no FHIR R4 conversion: it lives only in `health-fhir-mapping` on the server (ADRs 0015, 0050;
//   #853). R4-only Questionnaire item types and R4 element names are the tell.
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

/** What only R4 → R5 conversion code would name: R4's `open-choice` Questionnaire item type
 * (R5 has none; its report code is `open-choice-to-coding`), and a table named for R4. */
const r4Conversion: [RegExp, string][] = [
  [/['"`]open-choice['"`]/, "R4's open-choice item type"],
  [/\bR4_[A-Z]/, 'an R4 conversion table'],
]

/**
 * Files allowed to use browser storage: user-keyed UI preferences, the OAuth transaction (PKCE
 * verifier and state) kept across the sign-in redirect, which is removed at the callback, and the
 * device's enrollment key pair, whose private key the browser will not export (ADR 0009). None is
 * clinical data.
 */
const storageAllowlist = new Set(['platform/preferences.ts', 'platform/sign-in.ts', 'platform/device.ts'])

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
    for (const [pattern, what] of r4Conversion) {
      if (pattern.test(line)) problems.push(`${at}: ${what}: FHIR R4 is converted on the server by health-fhir-mapping, never in the client (ADRs 0015, 0050)`)
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
