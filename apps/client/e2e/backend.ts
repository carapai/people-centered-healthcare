// The e2e backend (#228): SurrealDB in memory, central-server with sign-in, the access policy,
// the client registry and the patient portal, and two provisioned users. Playwright starts it (playwright.config.ts) and
// stops it with the run. Binaries come from SURREAL_BIN (default `surreal`) and
// CENTRAL_SERVER_BIN (default the workspace's debug build).
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
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
      roles: { clerk: ['Patient.crs', 'caregiver.manage', 'encounters.checkin', 'queues.view'], reviewer: ['Patient.crus', 'registry.review'], storekeeper: ['stock.manage'], cashier: ['payments.cash'], supervisor: ['payments.cash', 'payments.supervise'], nurse: ['workflow.work', 'Questionnaire.rs', 'QuestionnaireResponse.crus', 'Patient.crs', 'queues.view', 'queues.work', 'Observation.crus', 'encounters.manage'], clinician: ['Patient.rs', 'ServiceRequest.rs', 'Appointment.rs', 'AllergyIntolerance.rs', 'Condition.rs', 'MedicationRequest.rs', 'Observation.rs', 'Encounter.rs', 'EpisodeOfCare.rs', 'queues.view', 'queues.work', 'encounters.manage', 'consultation.document', 'orders.request', 'medications.prescribe', 'results.review', 'workflow.work', 'DiagnosticReport.rs', 'Composition.rs', 'careplans.manage'], pharmacist: ['Patient.rs', 'pharmacy.view', 'pharmacy.dispense'], labtech: ['Patient.rs', 'workflow.work', 'laboratory.work', 'laboratory.verify'] },
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
