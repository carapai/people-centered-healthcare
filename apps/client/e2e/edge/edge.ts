// A facility-edge process for the edge suite (#76): the workspace's build (or FACILITY_EDGE_BIN),
// with the web app embedded, over a fresh data directory. Tests start and stop it themselves, so
// a test can take the edge away from a loaded page.
import { spawn } from 'node:child_process'
import { mkdtempSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const binary = process.env['FACILITY_EDGE_BIN'] ?? resolve(import.meta.dirname, '../../../../target/debug/facility-edge')

export interface Edge {
  readonly origin: string
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

export async function startEdge(): Promise<Edge> {
  const port = await freePort()
  const origin = `http://127.0.0.1:${port}`
  const child = spawn(binary, [], {
    env: {
      HEALTH_DATA_DIR: join(mkdtempSync(join(tmpdir(), 'health-edge-e2e-')), 'store'),
      HEALTH_BIND: `127.0.0.1:${port}`,
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
  return { origin, stop }
}
