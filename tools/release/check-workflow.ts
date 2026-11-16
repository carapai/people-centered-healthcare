// Holds release.yml to "signed or failed" (ADR 0045), and tests the gate it runs.
// Usage: bun tools/release/check-workflow.ts   (from the repository root; the release-check workflow)
//
// - Every job is in the `release` environment and runs check-signing.sh before anything but the
//   checkout, over every environment variable (`toJSON(vars)`).
// - Every setting the workflow reads is one the gate checks.
// - `publish` needs every other job, and only it may write to the repository or a registry, or
//   push, sign into a transparency log or create the release.
// - The updater key never reaches a step's environment or Vite, and updater signatures are made
//   after Authenticode (tauri.conf.json creates none at bundle time).
// - The gate fails, naming them, when any setting is missing, and passes when all are present.

import { readFileSync } from 'node:fs'

type Step = { uses?: string; run?: string; name?: string; env?: Record<string, string> }
type Job = {
  environment?: string | { name: string }
  needs?: string | string[]
  permissions?: Record<string, string>
  steps: Step[]
}
type Workflow = { on: Record<string, unknown>; permissions: Record<string, string>; jobs: Record<string, Job> }

const problems: string[] = []
const fail = (message: string) => problems.push(message)

const text = readFileSync('.github/workflows/release.yml', 'utf8')
const workflow = Bun.YAML.parse(text) as Workflow
const gate = 'tools/release/check-signing.sh'
const list = Bun.spawnSync(['bash', gate, '--list'])
const required = new Set(new TextDecoder().decode(list.stdout).trim().split('\n'))
if (required.size < 10) fail(`${gate} --list printed ${required.size} names`)

if (JSON.stringify(Object.keys(workflow.on)) !== '["push"]') fail('release.yml runs on anything but a tag push')
if (JSON.stringify(workflow.permissions) !== '{"contents":"read"}') fail('the workflow default is not contents: read')

const publishing = /docker push|docker login|gh release|cosign|attest|upload-release/
const jobs = Object.entries(workflow.jobs)
for (const [id, job] of jobs) {
  const environment = typeof job.environment === 'string' ? job.environment : job.environment?.name
  if (environment !== 'release') fail(`${id} is not in the release environment`)
  const first = job.steps.findIndex((step) => !step.uses?.startsWith('actions/checkout@'))
  const step = job.steps[first]
  if (step?.run?.trim() !== gate || step.env?.RELEASE_VARS !== '${{ toJSON(vars) }}') {
    fail(`${id}: its first step after the checkout is not the gate over toJSON(vars)`)
  }
  if (id === 'publish') continue
  for (const scope of ['contents', 'packages', 'attestations']) {
    if (job.permissions?.[scope] === 'write') fail(`${id} may write ${scope}; only publish may`)
  }
  for (const later of job.steps) {
    const what = `${later.uses ?? ''} ${later.run ?? ''}`
    if (publishing.test(what)) fail(`${id} publishes (${what.match(publishing)?.[0]}); only publish may`)
  }
}
const publish = workflow.jobs.publish
if (publish === undefined) fail('there is no publish job')
else {
  const needs = new Set([publish.needs ?? []].flat())
  for (const [id] of jobs) if (id !== 'publish' && !needs.has(id)) fail(`publish does not need ${id}`)
}

// The updater key reaches only tools/release/sign-updater.sh's process: never a step's
// environment, and never Vite, which tauri.conf.json would run if it had a before-command.
if (/TAURI_SIGNING/.test(text)) fail('a step sets TAURI_SIGNING_*; only sign-updater.sh may hold the key')
const tauri = JSON.parse(readFileSync('apps/client/src-tauri/tauri.conf.json', 'utf8'))
for (const hook of ['beforeBuildCommand', 'beforeBundleCommand', 'beforeDevCommand']) {
  if (hook in (tauri.build ?? {})) fail(`tauri.conf.json has ${hook}; the webview is built before the keys are fetched`)
}
if (tauri.bundle?.createUpdaterArtifacts !== false) {
  fail('tauri.conf.json creates updater artifacts at bundle time, before Authenticode changes the bytes')
}

for (const [, name] of text.matchAll(/\bvars\.([A-Z0-9_]+)/g)) {
  if (!required.has(name)) fail(`release.yml reads vars.${name}, which the gate does not check`)
}

// The gate itself.
const run = (env: Record<string, string>) => {
  const result = Bun.spawnSync(['bash', gate], { env: { PATH: process.env.PATH ?? '', ...env } })
  return { code: result.exitCode, err: new TextDecoder().decode(result.stderr) }
}
const all = Object.fromEntries([...required].map((name) => [name, 'set']))
const none = run({})
if (none.code === 0) fail('the gate passes with nothing set')
for (const name of required) if (!none.err.includes(name)) fail(`the gate does not name ${name} when it is missing`)
if (run(all).code !== 0) fail('the gate fails with everything set')
if (run({ RELEASE_VARS: JSON.stringify(all) }).code !== 0) fail('the gate fails with everything in RELEASE_VARS')
const [last] = [...required].slice(-1)
const one = run({ RELEASE_VARS: JSON.stringify({ ...all, [last as string]: '' }) })
if (one.code === 0 || !one.err.includes(last as string)) fail(`the gate passes without ${last}`)

if (problems.length > 0) {
  for (const problem of problems) console.error(`release.yml: ${problem}`)
  process.exit(1)
}
console.log(`release.yml: ${jobs.length} jobs gated on ${required.size} signing settings`)
