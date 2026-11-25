// A facility-edge process for the edge suite (#76): the workspace's build (or FACILITY_EDGE_BIN),
// with the web app embedded, over a fresh data directory. Tests start and stop it themselves, so
// a test can take the edge away from a loaded page.
import { type ChildProcess, spawn, spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const binary = process.env['FACILITY_EDGE_BIN'] ?? resolve(import.meta.dirname, '../../../../target/debug/facility-edge')
// Plays central for sign-in (#75): `cargo build -p facility-edge --example edge-fixture`.
const fixture = process.env['EDGE_FIXTURE_BIN'] ?? resolve(import.meta.dirname, '../../../../target/debug/examples/edge-fixture')

/** The member of staff central provisions to an edge started with `signIn`. */
export const staff = { username: 'nakato', password: 'tumbling otter quarry 7919' }

/** How many first ANC visits an edge started with `dashboards` holds, all today. */
export const firstVisits = 3

export interface Edge {
  readonly origin: string
  /** Central's provisioning bundle for `staff`, when the edge serves sign-in. */
  readonly bundle?: string
  stop(): Promise<void>
}

// Every edge this worker started and has not seen exit. A test that times out while its edge is
// starting never gets an `Edge` to stop, and Playwright then tears the worker down: the edges
// still here are killed when the worker exits, so none outlives the run (#907).
const running = new Set<ChildProcess>()
process.once('exit', () => {
  for (const child of running) child.kill('SIGKILL')
})

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
export async function startEdge(options: { signIn?: boolean; opd?: boolean; dashboards?: boolean; boundForm?: boolean } = {}): Promise<Edge> {
  // An unprovisioned edge serves only its console, the plain-HTTP loopback listener for its own
  // screen; the LAN port stays closed until it has a certificate. Devices' DPoP proofs name the
  // console's origin, so it is one the edge accepts.
  const port = await freePort()
  const origin = `http://127.0.0.1:${port}`
  const dir = mkdtempSync(join(tmpdir(), 'health-edge-e2e-'))
  let auth: Record<string, string> = {}
  let bundle: string | undefined
  if (options.signIn === true) {
    // With `opd`, the OPD package's definitions (the triage form and its mapping) are in the
    // edge's store, as central's sync would bring them (#106).
    const opd = options.opd === true ? [resolve(import.meta.dirname, '../../../../packages/reference-packages/opd')] : []
    // With `boundForm` (#843), a sample package whose form binds its questions to ValueSets
    // (e2e/fixtures/bound-form): its definitions in the store, as sync brings them, and the
    // package active, so the edge serves its terminology.
    const boundDir = resolve(import.meta.dirname, '../fixtures/bound-form')
    const bound = options.boundForm === true ? [boundDir, '--activate', boundDir] : []
    // With `dashboards` (#879), the sample indicators, MoH's Age Band Sets and the sample Reporting
    // package with the ANC overview Dashboard are active, as central's activation would leave the
    // node, and `firstVisits` women each had a first ANC visit here today.
    const e2e = resolve(import.meta.dirname, '..')
    const dashboards =
      options.dashboards === true
        ? [
            ...['fixtures/indicators', '../../../packages/reference-packages/core', 'fixtures/dashboards'].flatMap((p) => ['--activate', resolve(e2e, p)]),
            '--first-visits',
            String(firstVisits),
          ]
        : []
    const made = spawnSync(fixture, [dir, 'ug-kla-0001', 'KLA001', staff.username, staff.password, ...opd, ...bound, ...dashboards], { stdio: 'inherit' })
    if (made.status !== 0) throw new Error('edge-fixture failed: run `cargo build -p facility-edge --example edge-fixture`')
    auth = {
      HEALTH_NODE_ID: 'ug-kla-0001',
      HEALTH_FACILITY: 'KLA001',
      HEALTH_SNAPSHOT_KEYS_FILE: join(dir, 'snapshot-keys.json'),
      // Registration on the edge (#106), its MRNs under the edge's own code.
      HEALTH_NODE_CODE: 'E1',
      HEALTH_REGISTRY_NIN_SYSTEM: 'urn:health:nin',
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
  running.add(child)
  const exited = new Promise<void>((done) => child.once('exit', () => done()))
  void exited.then(() => running.delete(child))
  const stop = async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM')
    await exited
  }
  try {
    // A fresh store's first start runs every migration, over a minute on macOS (ADR 0060).
    const deadline = Date.now() + 300_000
    for (;;) {
      if (child.exitCode !== null) throw new Error(`facility-edge exited with ${child.exitCode}`)
      const ready = await fetch(`${origin}/ready`).then((r) => r.ok, () => false)
      if (ready) break
      if (Date.now() > deadline) throw new Error('facility-edge was not ready in time')
      await new Promise((done) => setTimeout(done, 250))
    }
    if (!(await fetch(`${origin}/sw.js`)).ok) {
      throw new Error('facility-edge was built without the web app: run `bun run build`, then `cargo build -p facility-edge`')
    }
  } catch (error) {
    // Whatever went wrong, the caller has no edge to stop: stop the one started here.
    await stop()
    throw error
  }
  return { origin, ...(bundle === undefined ? {} : { bundle }), stop }
}
