// Production bundle scan (client-application.md, ADR 0054). Fails when the web bundle contains
// Tauri or devtools code, or when Panda emitted CSS without token variables.
// Usage: bun scripts/scan-bundle.ts dist

import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'

const distDir = process.argv[2] ?? 'dist'

const forbiddenInJs: [RegExp, string][] = [
  [/@tauri-apps|__TAURI_INTERNALS__|plugin:|ipc:\/\//, 'Tauri code'],
  [/react-query-devtools|router-devtools|TanStackDevtools/, 'TanStack devtools'],
]

async function* files(dir: string): AsyncGenerator<string> {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) yield* files(path)
    else yield path
  }
}

const problems: string[] = []
let cssFiles = 0
let tokenVariables = 0

for await (const path of files(distDir)) {
  if (path.endsWith('.js')) {
    const text = await readFile(path, 'utf8')
    for (const [pattern, what] of forbiddenInJs) {
      if (pattern.test(text)) problems.push(`${path}: contains ${what}`)
    }
  } else if (path.endsWith('.css')) {
    cssFiles += 1
    const text = await readFile(path, 'utf8')
    tokenVariables += (text.match(/var\(--(colors|spacing|font-sizes|radii)-/g) ?? []).length
    // Without presets Panda writes shorthand properties verbatim, e.g. `.p_6{p:6}`.
    const raw = text.match(/\{\s*(p|px|py|m|bg|fontSize)\s*:/)
    if (raw !== null) problems.push(`${path}: Panda emitted raw shorthand \`${raw[0]}\`; check the presets`)
    // A token name the theme does not define is emitted verbatim, e.g. `z-index:modal`.
    const unresolved = text.match(/z-index:\s*(?!auto|inherit|initial|unset|revert|var\()[a-z][\w-]*/)
    if (unresolved !== null) problems.push(`${path}: unresolved token \`${unresolved[0]}\`; define it in the design system`)
  }
}

if (cssFiles === 0) problems.push(`${distDir}: no CSS emitted`)
else if (tokenVariables === 0) problems.push(`${distDir}: CSS contains no Panda token variables`)

for (const problem of problems) console.error(problem)
if (problems.length > 0) {
  console.error(`scan-bundle: ${problems.length} problem(s)`)
  process.exit(1)
}
console.log(`scan-bundle: ok (${tokenVariables} token variable uses in ${cssFiles} CSS file(s))`)
