// The e2e backend (#228): SurrealDB in memory, central-server with sign-in, the access policy,
// the client registry and the patient portal, and the provisioned users (e2e/users.ts), among
// them `admin` with every right. Playwright starts it (playwright.config.ts) and stops it with
// the run. Binaries come from SURREAL_BIN (default `surreal`) and CENTRAL_SERVER_BIN (default
// the workspace's debug build).
//
// Run by hand with HEALTH_SEED=1 (or a patient count, e.g. HEALTH_SEED=1000), it also loads a
// small synthetic world as the admin, with visits, vital signs, stock and appointments at KAM013
// (e2e/seed.ts, e2e/activity.ts; SYNTHETIC_DATA_BIN, default the workspace's debug build of
// tools/synthetic-data). Everything is in memory: it is seeded again each start.
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { activity } from './activity'
import { seed, signIn } from './seed'
import { ADMIN_PERMISSIONS, OTP_OUTBOX, USERS } from './users'

// Ports may be moved, e.g. to run a second backend beside one already running, with the app's
// origin for its sign-in redirects.
const ORIGIN = process.env['HEALTH_E2E_ORIGIN'] ?? 'http://localhost:4173'
const CENTRAL_PORT = process.env['HEALTH_E2E_CENTRAL_PORT'] ?? '18080'
const DB_PORT = process.env['HEALTH_E2E_DB_PORT'] ?? '18000'
const CENTRAL = `http://127.0.0.1:${CENTRAL_PORT}`

const surrealBin = process.env['SURREAL_BIN'] ?? 'surreal'
const centralBin = process.env['CENTRAL_SERVER_BIN'] ?? resolve(import.meta.dirname, '../../../target/debug/central-server')
const syntheticBin = process.env['SYNTHETIC_DATA_BIN'] ?? resolve(import.meta.dirname, '../../../target/debug/synthetic-data')
const seedPatients = process.env['HEALTH_SEED'] === '1' ? 300 : Number(process.env['HEALTH_SEED'] ?? 0)
const dir = mkdtempSync(join(tmpdir(), 'health-e2e-'))
const file = (name: string, content: string) => {
  const path = join(dir, name)
  writeFileSync(path, content, { mode: 0o600 })
  return path
}
const otpOutbox = process.env['HEALTH_DEV_OTP_OUTBOX'] ?? OTP_OUTBOX
writeFileSync(otpOutbox, '', { mode: 0o600 })
const dbPassword = randomBytes(16).toString('hex')
// A throwaway publication-signing key and self-signed certificate, so the Studio can publish
// locally (#835). With no national root configured, nothing verifies the chain against one.
const publication = (() => {
  const key = join(dir, 'publication-key.pem')
  const chain = join(dir, 'publication-chain.pem')
  const made =
    spawnSync('openssl', ['genpkey', '-algorithm', 'EC', '-pkeyopt', 'ec_paramgen_curve:P-256', '-out', key]).status === 0 &&
    spawnSync('openssl', ['req', '-new', '-x509', '-key', key, '-subj', '/CN=e2e publication signing', '-days', '30', '-out', chain]).status === 0
  if (made) return { HEALTH_PUBLICATION_KEY_FILE: key, HEALTH_PUBLICATION_CHAIN_FILE: chain }
  process.stdout.write('openssl did not make a publication-signing key: the Studio cannot publish\n')
  return {}
})()
const env = {
  ...process.env,
  ...publication,
  HEALTH_BIND: `127.0.0.1:${CENTRAL_PORT}`,
  HEALTH_SURREALDB_ENDPOINT: `ws://127.0.0.1:${DB_PORT}`,
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
  HEALTH_DEV_OTP_OUTBOX: otpOutbox,
  // Central's blob store, which holds the Studio's package tarballs (#492).
  HEALTH_FILES_DIR: (() => {
    const files = join(dir, 'files')
    mkdirSync(files)
    return files
  })(),
  // Payments at one till (#254).
  HEALTH_PAYMENT_TILLS: 'till-1',
  // The ANC visit form (#92), and the OPD triage form with its vital-signs mapping (#103) from
  // the ug.moh.opd reference package, each under a UUID id, for nodes without activation (#89).
  HEALTH_DEV_FHIR_SEED: file(
    'fhir-seed.json',
    JSON.stringify({
      resourceType: 'Bundle',
      type: 'collection',
      entry: [
        {
          resource: {
            resourceType: 'Questionnaire',
            id: '0190f1a2-7c3d-7e4f-8a5b-6c7d8e9f0a1b',
            url: 'https://health.go.ug/fhir/Questionnaire/anc-visit',
            version: '2.0.0',
            title: 'ANC visit',
            status: 'active',
            item: [
              { linkId: 'age', text: 'Age', type: 'integer', required: true },
              {
                linkId: 'adolescent-counselling',
                text: 'Adolescent counselling given',
                type: 'boolean',
                extension: [
                  {
                    url: 'http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-enableWhenExpression',
                    valueExpression: { language: 'text/fhirpath', expression: "%resource.item.where(linkId = 'age').answer.value < 18" },
                  },
                ],
              },
              { linkId: 'pregnant', text: 'Pregnant', type: 'boolean' },
              { linkId: 'lmp', text: 'Last menstrual period', type: 'date', required: true, enableWhen: [{ question: 'pregnant', operator: '=', answerBoolean: true }] },
            ],
          },
        },
        ...[
          ['Questionnaire-opd-triage.json', '0190f1a2-7c3d-7e4f-8a5b-0000000103a1'],
          ['CodeSystem-triage-category.json', '0190f1a2-7c3d-7e4f-8a5b-0000000103a2'],
          ['Library-opd-triage-mapping.json', '0190f1a2-7c3d-7e4f-8a5b-0000000103a3'],
        ].map(([path, id]) => ({
          resource: { ...JSON.parse(readFileSync(new URL(`../../../packages/reference-packages/opd/${path}`, import.meta.url), 'utf8')), id },
        })),
      ],
    }),
  ),
  // An OPD visit's triage and consultation worklists (#99), until activation brings published
  // definitions (#89).
  HEALTH_DEV_WORKFLOW_DEFINITIONS: file(
    'workflow-definitions.json',
    JSON.stringify([
      {
        url: 'https://health.go.ug/fhir/Library/opd-visit',
        version: '1.0.0',
        statechart: {
          states: [
            { id: 'triage', kind: 'atomic', task: { role: 'nurse', worklist: 'opd-triage' }, transitions: [{ events: ['triaged'], targets: ['consultation'] }] },
            { id: 'consultation', kind: 'atomic', task: { role: 'clinician', worklist: 'opd-consult', priority: 'urgent' }, transitions: [{ events: ['seen'], targets: ['done'] }] },
            { id: 'done', kind: 'final' },
          ],
        },
      },
      // The laboratory's (#109), from the ug.moh.lab reference package.
      JSON.parse(readFileSync(new URL('../../../packages/reference-packages/sources/lab-test.json', import.meta.url), 'utf8')),
    ]),
  ),
  HEALTH_ACCESS_POLICY_FILE: file(
    'access-policy.json',
    JSON.stringify({
      roles: { clerk: ['Patient.crs', 'caregiver.manage', 'encounters.checkin', 'queues.view'], reviewer: ['Patient.crus', 'registry.review'], storekeeper: ['stock.manage'], cashier: ['payments.cash'], supervisor: ['payments.cash', 'payments.supervise'], nurse: ['workflow.work', 'Questionnaire.rs', 'QuestionnaireResponse.crus', 'Patient.crs', 'queues.view', 'queues.work', 'Observation.crus', 'encounters.manage'], clinician: ['Patient.rs', 'ServiceRequest.rs', 'Appointment.rs', 'AllergyIntolerance.rs', 'Condition.rs', 'MedicationRequest.rs', 'Observation.rs', 'Encounter.rs', 'EpisodeOfCare.rs', 'queues.view', 'queues.work', 'encounters.manage', 'consultation.document', 'orders.request', 'medications.prescribe', 'results.review', 'workflow.work', 'DiagnosticReport.rs', 'Composition.rs', 'careplans.manage'], pharmacist: ['Patient.rs', 'pharmacy.view', 'pharmacy.dispense'], labtech: ['Patient.rs', 'workflow.work', 'laboratory.work', 'laboratory.verify'], studio: ['metadata.review', 'metadata.approve'], author: ['metadata.author'], approver: ['metadata.approve'], publisher: ['metadata.publish', 'configuration.activate'], admin: ADMIN_PERMISSIONS },
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

children.push(spawn(surrealBin, ['start', '--log', 'warn', '--bind', `127.0.0.1:${DB_PORT}`, '--user', 'root', '--pass', dbPassword, 'memory'], { stdio: 'inherit' }))
await waitFor(`http://127.0.0.1:${DB_PORT}/health`, 30_000)
for (const user of Object.values(USERS)) {
  const provisioned = spawnSync(centralBin, ['provision', user.username, 'KAM013', user.roles], {
    env: { ...env, HEALTH_PROVISION_PASSWORD_FILE: file(`${user.username}-password`, user.password) },
    stdio: 'inherit',
  })
  if (provisioned.status !== 0) throw new Error(`provisioning ${user.username} failed`)
}
children.push(spawn(centralBin, [], { env, stdio: 'inherit' }))
await waitFor(`${CENTRAL}/ready`, 60_000)
// Sign-in and the FHIR API answer 503 until they are built, just after /ready.
for (let tries = 0; (await fetch(`${CENTRAL}/fhir/metadata`).catch(() => undefined))?.status === 503; tries += 1) {
  if (tries > 120) throw new Error('the FHIR API did not come up')
  await new Promise((r) => setTimeout(r, 250))
}
// A draft of the OPD reference package for the Studio (#492), by e2e-author, so a reviewer sees
// it waiting for its author, and its author can sign in and submit it (#835). The lifecycle
// records people by account id: the access token's subject.
const authorToken = await signIn(CENTRAL, ORIGIN, USERS.author, 'KAM013')
const author = (JSON.parse(Buffer.from(authorToken.split('.')[1] ?? '', 'base64url').toString()) as { sub: string }).sub
const drafted = spawnSync(centralBin, ['package', 'draft', resolve(import.meta.dirname, '../../../packages/reference-packages/opd'), '--by', author], { env, stdio: 'inherit' })
if (drafted.status !== 0) throw new Error('drafting the Studio package failed')
// A synthetic ART program (#500), by the same author, for the Program Designer: its care
// workflow's states, regimen templates, and a form, an ActivityDefinition and a Measure it names
// but does not hold yet (e2e/fixtures/art). Not clinical guidance: the reference package is #117.
const program = spawnSync(centralBin, ['package', 'draft', resolve(import.meta.dirname, 'fixtures/art'), '--by', author], { env, stdio: 'inherit' })
if (program.status !== 0) throw new Error('drafting the ART program package failed')
// Published packages (#855): one with a workflow, so the Studio's sections show a published design
// that Edit starts the next version of (e2e/fixtures/clinic), and the national core with MoH's Age
// Band Sets (#551; packages/reference-packages/core). Each is drafted by the author, reviewed and
// approved by two others, and published; it needs the publication-signing key made above.
if ('HEALTH_PUBLICATION_KEY_FILE' in publication) {
  for (const [source, id] of [
    [resolve(import.meta.dirname, 'fixtures/clinic'), 'ug.moh.clinic@1.0.0'],
    [resolve(import.meta.dirname, '../../../packages/reference-packages/core'), 'ug.moh.core@1.0.0'],
  ] as const) {
    for (const step of [
      ['draft', source, '--by', author],
      ['submit', id, '--by', author],
      ['review', id, '--by', 'e2e-reviewer-cli'],
      ['approve', id, '--by', 'e2e-approver-cli'],
      ['approve', id, '--by', 'e2e-second-approver-cli'],
      ['publish', id, '--by', 'e2e-publisher-cli'],
    ]) {
      const done = spawnSync(centralBin, ['package', ...step], { env, stdio: 'inherit' })
      if (done.status !== 0) throw new Error(`${step[0]} of ${id} failed`)
    }
  }
}
// The ANC reference package (#548–#552; packages/reference-packages/anc), drafted by the author:
// its indicators, disaggregated by the national core's ANC age bands published above, and its test
// patients, which the indicator editor's Try evaluates (#537).
const anc = spawnSync(centralBin, ['package', 'draft', resolve(import.meta.dirname, '../../../packages/reference-packages/anc'), '--by', author], { env, stdio: 'inherit' })
if (anc.status !== 0) throw new Error('drafting the ANC reference package failed')
// The HMIS Reporting package (#556; packages/reference-packages/hmis), drafted by the author: the
// HMIS 105 antenatal care Report and the facility's ANC monitoring, over the ANC indicators.
const hmis = spawnSync(centralBin, ['package', 'draft', resolve(import.meta.dirname, '../../../packages/reference-packages/hmis'), '--by', author], { env, stdio: 'inherit' })
if (hmis.status !== 0) throw new Error('drafting the HMIS reference package failed')
if (seedPatients > 0) {
  const token = await signIn(CENTRAL, ORIGIN, USERS.admin, 'KAM013')
  const generated = seed(syntheticBin, dir, CENTRAL, token, seedPatients)
  process.stdout.write(`${await activity(CENTRAL, token, generated)}\n`)
  process.stdout.write(`seeded ${seedPatients} patients; sign in as ${USERS.admin.username} / ${USERS.admin.password} at KAM013\n`)
}
process.stdout.write('e2e backend ready\n')
