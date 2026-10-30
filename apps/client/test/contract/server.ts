// Starts health-fhir-server's contract_server example: a central and an edge node, each on its
// own memory database and port. CONTRACT_SERVERS='{"central":…,"edge":…}' points the suite at
// running nodes instead.

import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { fileURLToPath } from 'node:url'

import type { TestProject } from 'vitest/node'

import type { ContractServers } from './context'

export default async function setup(project: TestProject): Promise<(() => void) | undefined> {
  const given = process.env['CONTRACT_SERVERS']
  if (given !== undefined) {
    project.provide('servers', JSON.parse(given) as ContractServers)
    return undefined
  }
  const root = fileURLToPath(new URL('../../../../', import.meta.url))
  const child = spawn('cargo', ['run', '--locked', '--quiet', '-p', 'health-fhir-server', '--example', 'contract_server'], {
    cwd: root,
    stdio: ['ignore', 'pipe', 'inherit'],
  })
  const line = await new Promise<string>((resolve, reject) => {
    child.once('error', reject)
    child.once('exit', (code) => reject(new Error(`contract_server exited with ${code}`)))
    createInterface({ input: child.stdout }).once('line', resolve)
  })
  project.provide('servers', JSON.parse(line) as ContractServers)
  return () => {
    child.kill()
  }
}
