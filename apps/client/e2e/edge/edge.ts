// A facility-edge process for the edge suite (#76): the workspace's build (or FACILITY_EDGE_BIN),
// with the web app embedded, over a fresh data directory. Tests start and stop it themselves, so
// a test can take the edge away from a loaded page.
import { spawn, spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const binary = process.env['FACILITY_EDGE_BIN'] ?? resolve(import.meta.dirname, '../../../../target/debug/facility-edge')
// Plays central for sign-in (#75): `cargo build -p facility-edge --example edge-fixture`.
const fixture = process.env['EDGE_FIXTURE_BIN'] ?? resolve(import.meta.dirname, '../../../../target/debug/examples/edge-fixture')

/** The member of staff central provisions to an edge started with `signIn`. */
export const staff = { username: 'nakato', password: 'tumbling otter quarry 7919' }

export interface Edge {
  readonly origin: string
  /** Central's provisioning bundle for `staff`, when the edge serves sign-in. */
  readonly bundle?: string
  stop(): Promise<void>
}

function freePort(): Promise<number> {
  return new Promise((done, fail) => {
    const server = createServer()
    server.once('error', fail)
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      server.close(() => (typeof address === 'object' && address !== null ? done(address.port) : fail(new Error('no port'))))
    })
  })
}

/**
 * Starts an edge. With `signIn`, it also serves staff sign-in: the keys `provision request` would
 * seal, central's snapshot keys, and a bundle for `staff` come from `edge-fixture`.
 */
export async function startEdge(options: { signIn?: boolean } = {}): Promise<Edge> {
  // An unprovisioned edge serves only its console, the plain-HTTP loopback listener for its own
  // screen; the LAN port stays closed until it has a certificate. Devices' DPoP proofs name the
  // console's origin, so it is one the edge accepts.
  const port = await freePort()
  const origin = `http://127.0.0.1:${port}`
  const dir = mkdtempSync(join(tmpdir(), 'health-edge-e2e-'))
  let auth: Record<string, string> = {}
  let bundle: string | undefined
  if (options.signIn === true) {
    const made = spawnSync(fixture, [dir, 'ug-kla-0001', 'ug-kla-mulago', staff.username, staff.password], { stdio: 'inherit' })
    if (made.status !== 0) throw new Error('edge-fixture failed: run `cargo build -p facility-edge --example edge-fixture`')
    auth = {
      HEALTH_NODE_ID: 'ug-kla-0001',
      HEALTH_FACILITY: 'ug-kla-mulago',
      HEALTH_SNAPSHOT_KEYS_FILE: join(dir, 'snapshot-keys.json'),
      CREDENTIALS_DIRECTORY: join(dir, 'credentials'),
    }
    bundle = readFileSync(join(dir, 'bundle.jws'), 'utf8')
  }
  const child = spawn(binary, [], {
    env: {
      ...auth,
      HEALTH_DATA_DIR: join(dir, 'store'),
      HEALTH_CONSOLE_BIND: `127.0.0.1:${port}`,
      HEALTH_BIND: `127.0.0.1:${await freePort()}`,
      HEALTH_ORIGINS: origin,
      HEALTH_MDNS: 'off',
      HEALTH_DRAIN_SECS: '2',
      HEALTH_LOG: 'warn',
    },
    stdio: ['ignore', 'ignore', 'inherit'],
  })
  const exited = new Promise<void>((done) => child.once('exit', () => done()))
  const stop = async () => {
    if (child.exitCode === null) child.kill('SIGTERM')
    await exited
  }
  const deadline = Date.now() + 120_000
  for (;;) {
    if (child.exitCode !== null) throw new Error(`facility-edge exited with ${child.exitCode}`)
    const ready = await fetch(`${origin}/ready`).then((r) => r.ok, () => false)
    if (ready) break
    if (Date.now() > deadline) {
      await stop()
      throw new Error('facility-edge was not ready in time')
    }
    await new Promise((done) => setTimeout(done, 250))
  }
  if (!(await fetch(`${origin}/sw.js`)).ok) {
    await stop()
    throw new Error('facility-edge was built without the web app: run `bun run build`, then `cargo build -p facility-edge`')
  }
  return { origin, ...(bundle === undefined ? {} : { bundle }), stop }
}
