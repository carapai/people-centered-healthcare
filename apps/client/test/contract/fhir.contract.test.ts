// The FhirClient contract (client-application.md § Ports): one suite, run against central (OIDC
// bearer), an edge (DPoP) and a desktop over IPC (ADR 0036), over the real Rust FhirServer. The
// desktop is read through the client's own IPC transport into health-desktop-runtime's dispatch,
// so both halves of the IPC boundary meet the same contract as HTTP.

import type { Observation, Patient } from '@health/fhir-types'
import { isLosslessNumber } from 'lossless-json'
import { describe, expect, inject, test } from 'vitest'

import { bearerAuth, dpopAuth } from '../../src/platform/auth'
import { FhirError, fhirClient } from '../../src/platform/fhir'
import { httpTransport } from '../../src/platform/http'
import { parseJson, stringifyJson } from '../../src/platform/json'
import { memorySession } from '../../src/platform/session'
import { ipcTransport, type Invoke } from '../../src/platform/tauri'
import type { Transport } from '../../src/platform/types'
import type { ContractServers } from './context'

const servers: ContractServers = inject('servers')

async function deviceKey(): Promise<CryptoKeyPair> {
  return crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign', 'verify'])
}

describe.each([
  { node: 'central', base: servers.central, scheme: 'Bearer' },
  { node: 'edge', base: servers.edge, scheme: 'DPoP' },
  { node: 'desktop', base: servers.desktop.base, scheme: 'IPC' },
])('FhirClient against $node', async ({ node, base, scheme }) => {
  const sent: Headers[] = []
  const recording: typeof fetch = (input, init) => {
    sent.push(new Headers(init?.headers))
    return fetch(input, init)
  }
  // The desktop's `invoke('api_request', …)`, carried to the Rust dispatch: what the webview sends.
  const invoking: Invoke = async <T,>(command: string, args: Record<string, unknown>): Promise<T> => {
    expect(command).toBe('api_request')
    sent.push(new Headers((args['envelope'] as { headers: Record<string, string> }).headers))
    const response = await fetch(servers.desktop.ipc, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(args),
    })
    return (await response.json()) as T
  }
  const session = memorySession()
  session.setAccessToken(`contract-${node}`)
  const keys = await deviceKey()
  const url = new URL(base)
  const transport: Transport =
    scheme === 'IPC'
      ? ipcTransport(invoking)
      : httpTransport(url.origin, scheme === 'Bearer' ? bearerAuth(session) : dpopAuth(session, () => keys), recording)
  const fhir = fhirClient(transport, url.pathname)

  // Writes are not FhirClient's job (command ports own them), so the fixture is posted directly.
  async function create<T>(resource: object): Promise<T> {
    const type = (resource as { resourceType: string }).resourceType
    const response = await fetch(`${base}/${type}`, {
      method: 'POST',
      headers: { 'content-type': 'application/fhir+json' },
      body: stringifyJson(resource),
    })
    expect(response.status).toBe(201)
    return parseJson(await response.text()) as T
  }

  const system = 'urn:ietf:rfc:3986'
  const value = `urn:uuid:${crypto.randomUUID()}`
  const patient = await create<Patient>({
    resourceType: 'Patient',
    identifier: [{ system, value }],
    name: [{ family: 'Okello', given: ['Grace'] }],
    birthDate: '1990-04-02',
  })
  const observation = await create<Observation>(
    parseJson(`{
      "resourceType": "Observation",
      "status": "final",
      "code": { "text": "body weight" },
      "subject": { "reference": "Patient/${patient.id}" },
      "valueQuantity": { "value": 3.50, "unit": "kg" },
      "component": [{ "code": { "text": "precise" }, "valueQuantity": { "value": 0.1000000000000000000000000001 } }]
    }`) as object,
  )

  test('reads a resource', async () => {
    const read = await fhir.read('Patient', patient.id ?? '')
    expect(read.resourceType).toBe('Patient')
    expect(read.identifier?.[0]?.value).toBe(value)
  })

  test('decimals round-trip exactly', async () => {
    const read = await fhir.read('Observation', observation.id ?? '')
    const weight = read.valueQuantity?.value
    expect(isLosslessNumber(weight)).toBe(true)
    expect(weight?.toString()).toBe('3.50')
    expect(read.component?.[0]?.valueQuantity?.value?.toString()).toBe('0.1000000000000000000000000001')
    expect(stringifyJson(read.valueQuantity)).toBe('{"value":3.50,"unit":"kg"}')
  })

  test('reads a version', async () => {
    const versionId = observation.meta?.versionId ?? ''
    const version = await fhir.vread('Observation', observation.id ?? '', versionId)
    expect(version.meta?.versionId).toBe(versionId)
  })

  test.each(['GET', 'POST'] as const)('searches over %s', async (via) => {
    const bundle = await fhir.search('Patient', { identifier: `${system}|${value}` }, { via })
    expect(bundle.type).toBe('searchset')
    expect(bundle.entry?.map((entry) => entry.resource?.id)).toEqual([patient.id])
  })

  test('repeats a parameter', async () => {
    const bundle = await fhir.search('Patient', { identifier: `${system}|${value}`, birthdate: ['ge1990-01-01', 'le1990-12-31'] })
    expect(bundle.entry).toHaveLength(1)
  })

  test('reads instance and type history', async () => {
    const instance = await fhir.history({ type: 'Patient', id: patient.id ?? '' })
    expect(instance.type).toBe('history')
    expect(instance.entry).toHaveLength(1)
    const ofType = await fhir.history({ type: 'Observation' }, { _count: '50' })
    expect(ofType.entry?.some((entry) => entry.resource?.id === observation.id)).toBe(true)
  })

  test('a missing resource is a FhirError carrying the OperationOutcome', async () => {
    const error = await fhir.read('Patient', 'missing').catch((caught: unknown) => caught)
    expect(error).toBeInstanceOf(FhirError)
    expect((error as FhirError).status).toBe(404)
    expect((error as FhirError).outcome?.resourceType).toBe('OperationOutcome')
  })

  test(scheme === 'IPC' ? 'puts no credential in an IPC envelope' : `authorizes every request with ${scheme}`, () => {
    expect(sent.length).toBeGreaterThan(0)
    for (const headers of sent) {
      if (scheme === 'IPC') {
        // The desktop's session lives on the Rust side: nothing secret crosses into the webview.
        expect(headers.has('authorization')).toBe(false)
        expect(headers.has('dpop')).toBe(false)
      } else {
        expect(headers.get('authorization')).toBe(`${scheme} contract-${node}`)
        expect(headers.has('dpop')).toBe(scheme === 'DPoP')
      }
    }
  })
})

test('central and the edge are separate nodes', async () => {
  const central = fhirClient(httpTransport(new URL(servers.central).origin, bearerAuth(memorySession())))
  const response = await fetch(`${servers.edge}/Patient`, {
    method: 'POST',
    headers: { 'content-type': 'application/fhir+json' },
    body: '{"resourceType":"Patient"}',
  })
  const id = (parseJson(await response.text()) as Patient).id ?? ''
  await expect(central.read('Patient', id)).rejects.toMatchObject({ status: 404 })
})
