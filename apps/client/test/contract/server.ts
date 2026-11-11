// Starts the nodes the contract suite runs against: health-fhir-server's contract_server example
// (a central and an edge node, each on its own memory database and port) and
// health-desktop-runtime's ipc_server example (a desktop node reached over IPC).
// CONTRACT_SERVERS='{"central":…,"edge":…,"desktop":{"base":…,"ipc":…}}' points the suite at
// running nodes instead.

import { spawn, type ChildProcess } from 'node:child_process'
import { createInterface } from 'node:readline'
import { fileURLToPath } from 'node:url'

import type { TestProject } from 'vitest/node'

import type { ContractServers } from './context'

/** Runs `cargo run … --example <example>` and returns the JSON line it prints when ready. */
async function example(root: string, crate: string, name: string): Promise<{ child: ChildProcess; line: unknown }> {
  const child = spawn('cargo', ['run', '--locked', '--quiet', '-p', crate, '--example', name], {
    cwd: root,
    stdio: ['ignore', 'pipe', 'inherit'],
  })
  const line = await new Promise<string>((resolve, reject) => {
    child.once('error', reject)
    child.once('exit', (code) => reject(new Error(`${name} exited with ${code}`)))
    createInterface({ input: child.stdout }).once('line', resolve)
  })
  return { child, line: JSON.parse(line) }
}

export default async function setup(project: TestProject): Promise<(() => void) | undefined> {
  const given = process.env['CONTRACT_SERVERS']
  if (given !== undefined) {
    project.provide('servers', JSON.parse(given) as ContractServers)
    return undefined
  }
  const root = fileURLToPath(new URL('../../../../', import.meta.url))
  const nodes = await example(root, 'health-fhir-server', 'contract_server')
  const desktop = await example(root, 'health-desktop-runtime', 'ipc_server')
  project.provide('servers', {
    ...(nodes.line as Omit<ContractServers, 'desktop'>),
    desktop: desktop.line as ContractServers['desktop'],
  })
  return () => {
    nodes.child.kill()
    desktop.child.kill()
  }
}
