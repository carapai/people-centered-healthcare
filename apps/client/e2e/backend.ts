// The e2e backend (#228): SurrealDB in memory, central-server with sign-in, the access policy,
// the client registry and the patient portal, and two provisioned users. Playwright starts it (playwright.config.ts) and
// stops it with the run. Binaries come from SURREAL_BIN (default `surreal`) and
// CENTRAL_SERVER_BIN (default the workspace's debug build).
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { OTP_OUTBOX, USERS } from './users'

const ORIGIN = 'http://localhost:4173'
const CENTRAL = 'http://127.0.0.1:18080'

const surrealBin = process.env['SURREAL_BIN'] ?? 'surreal'
const centralBin = process.env['CENTRAL_SERVER_BIN'] ?? resolve(import.meta.dirname, '../../../target/debug/central-server')
const dir = mkdtempSync(join(tmpdir(), 'health-e2e-'))
const file = (name: string, content: string) => {
  const path = join(dir, name)
  writeFileSync(path, content, { mode: 0o600 })
  return path
}
writeFileSync(OTP_OUTBOX, '', { mode: 0o600 })
const dbPassword = randomBytes(16).toString('hex')
const env = {
  ...process.env,
  HEALTH_BIND: '127.0.0.1:18080',
  HEALTH_SURREALDB_ENDPOINT: 'ws://127.0.0.1:18000',
  HEALTH_SURREALDB_PASSWORD_FILE: file('db-password', dbPassword),
  HEALTH_LOG: 'warn',
  HEALTH_AUTH_ISSUER: `${ORIGIN}/auth`,
  HEALTH_AUTH_KEYS_FILE: file('auth-keys', `${randomBytes(32).toString('hex')}\n`),
  HEALTH_AUTH_SNAPSHOT_KEYS_FILE: file('snapshot-keys', `${randomBytes(32).toString('hex')}\n`),
  HEALTH_AUTH_REDIRECT_URIS: `${ORIGIN}/callback`,
  HEALTH_AUTH_LOGIN_PAGE: '/sign-in',
  HEALTH_REGISTRY_NIN_SYSTEM: 'urn:health:nin',
  // The patient portal (#111), sending one-time codes to the development outbox.
  HEALTH_PORTAL_REDIRECT_URIS: `${ORIGIN}/portal/callback`,
  HEALTH_DEV_OTP_OUTBOX: OTP_OUTBOX,
  HEALTH_ACCESS_POLICY_FILE: file(
    'access-policy.json',
    JSON.stringify({
      roles: { clerk: ['Patient.crs', 'caregiver.manage'], reviewer: ['Patient.crus', 'registry.review'] },
      labels: [],
    }),
  ),
}

const children: ChildProcess[] = []
const stop = () => {
  for (const child of children) child.kill('SIGTERM')
}
process.on('SIGINT', stop)
process.on('SIGTERM', () => {
  stop()
  process.exit(0)
})

async function waitFor(url: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      if ((await fetch(url)).ok) return
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 250))
  }
  throw new Error(`${url} did not come up`)
}

children.push(spawn(surrealBin, ['start', '--log', 'warn', '--bind', '127.0.0.1:18000', '--user', 'root', '--pass', dbPassword, 'memory'], { stdio: 'inherit' }))
await waitFor('http://127.0.0.1:18000/health', 30_000)
for (const user of Object.values(USERS)) {
  const provisioned = spawnSync(centralBin, ['provision', user.username, 'KAM013', user.roles], {
    env: { ...env, HEALTH_PROVISION_PASSWORD_FILE: file(`${user.username}-password`, user.password) },
    stdio: 'inherit',
  })
  if (provisioned.status !== 0) throw new Error(`provisioning ${user.username} failed`)
}
children.push(spawn(centralBin, [], { env, stdio: 'inherit' }))
await waitFor(`${CENTRAL}/ready`, 60_000)
process.stdout.write('e2e backend ready\n')
