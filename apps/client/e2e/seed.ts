// Seed data for a local backend (`HEALTH_SEED=1 bun e2e/backend.ts`): a small synthetic world
// from tools/synthetic-data (facilities with their stores, stock, medicines, a price book,
// payers and contracts, and patients by household), loaded through central's FHIR API as the
// admin. The e2e suite never sets HEALTH_SEED, so its runs start empty as before.
import { spawnSync } from 'node:child_process'
import { createHash, randomBytes } from 'node:crypto'
import { join } from 'node:path'

const b64url = (bytes: Buffer) => bytes.toString('base64url')

/** An access token for `username` at `facility`, by the web client's own authorization-code flow
 * with PKCE (central has no password grant). */
export async function signIn(
  central: string,
  origin: string,
  user: { username: string; password: string },
  facility: string,
): Promise<string> {
  const verifier = b64url(randomBytes(32))
  const request = {
    response_type: 'code',
    client_id: 'health-client',
    redirect_uri: `${origin}/callback`,
    scope: 'openid fhirUser user/*.rs',
    state: b64url(randomBytes(16)),
    code_challenge: b64url(createHash('sha256').update(verifier).digest()),
    code_challenge_method: 'S256',
  }
  const login = await fetch(`${central}/auth/authorize`, {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ ...request, username: user.username, password: user.password, facility }),
    redirect: 'manual',
  })
  const location = login.headers.get('location') ?? ''
  const code = new URL(location, origin).searchParams.get('code')
  if (code === null) throw new Error(`signing ${user.username} in did not give a code: ${login.status} ${location}`)
  const token = await fetch(`${central}/auth/token`, {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      grant_type: 'authorization_code',
      code,
      redirect_uri: request.redirect_uri,
      client_id: request.client_id,
      code_verifier: verifier,
    }),
  })
  const body = (await token.json()) as { access_token?: string }
  if (body.access_token === undefined) throw new Error(`the token request failed: ${token.status} ${JSON.stringify(body)}`)
  return body.access_token
}

/** Generates the synthetic world into `dir` and loads it into `central` with `token`. */
export function seed(syntheticBin: string, dir: string, central: string, token: string, patients: number): void {
  const out = join(dir, 'seed')
  const run = (args: string[]) => {
    const result = spawnSync(syntheticBin, args, { stdio: 'inherit' })
    if (result.status !== 0) throw new Error(`synthetic-data ${args[0]} failed`)
  }
  run(['generate', '--out', out, '--seed', '1', '--patients', String(patients)])
  run(['load', '--dir', out, '--base', `${central}/fhir`, '--bearer', token])
}
