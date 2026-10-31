import { describe, expect, test } from 'vitest'

import { candidates } from '../src/features/registration/candidates'
import { canRegister, emptyForm, toPatient } from '../src/features/registration/patient'
import { registriesCommands } from '../src/platform'
import { httpPlatform } from '../src/platform/compose'
import { fakeTransport } from './fake'

const NIN = 'urn:health:nin'

describe('registration', () => {
  test('the form becomes a staff-entered Patient; an age gives an estimated birth year', () => {
    const patient = toPatient(
      { ...emptyForm, family: ' Nakato ', given: 'Grace Mary', sex: 'female', age: '34', nin: 'cf12345678abcd', village: 'Kisugu' },
      NIN,
      2026,
    )
    expect(patient).toEqual({
      resourceType: 'Patient',
      meta: { tag: [{ system: 'urn:health:registry:trust', code: 'staff-entered' }] },
      name: [{ family: 'Nakato', given: ['Grace', 'Mary'] }],
      gender: 'female',
      birthDate: '1992',
      identifier: [{ system: NIN, value: 'CF12345678ABCD' }],
      address: [{ city: 'Kisugu' }],
    })
    // A known date wins over an age; empty fields are left out.
    expect(toPatient({ ...emptyForm, family: 'A', given: 'B', sex: 'male', birthDate: '1990-04-02', age: '5' }, NIN, 2026)).toEqual({
      resourceType: 'Patient',
      meta: { tag: [{ system: 'urn:health:registry:trust', code: 'staff-entered' }] },
      name: [{ family: 'A', given: ['B'] }],
      gender: 'male',
      birthDate: '1990-04-02',
    })
    expect(canRegister(emptyForm)).toBe(false)
    expect(canRegister({ ...emptyForm, family: 'A', given: 'B', sex: 'female' })).toBe(true)
  })

  test('$register asks first, then registers only when the clerk confirms', async () => {
    const replies = [
      {
        status: 200,
        body: {
          resourceType: 'Parameters',
          parameter: [
            {
              name: 'candidates',
              resource: {
                resourceType: 'Bundle',
                type: 'searchset',
                entry: [
                  {
                    resource: {
                      resourceType: 'Patient',
                      id: 'p1',
                      name: [{ family: 'Nakato', given: ['Grace'] }],
                      birthDate: '1990-04-02',
                      gender: 'female',
                      identifier: [{ type: { coding: [{ code: 'MR' }] }, value: 'KAM013-C1-000001' }],
                    },
                    search: { mode: 'match', score: 0.98, extension: [{ url: 'http://hl7.org/fhir/StructureDefinition/match-grade', valueCode: 'certain' }] },
                  },
                ],
              },
            },
          ],
        },
      },
      {
        status: 201,
        body: {
          resourceType: 'Parameters',
          parameter: [
            { name: 'patient', valueReference: { reference: 'Patient/p2' } },
            { name: 'mrn', valueString: 'KAM013-C1-000002' },
            { name: 'outcome', valueCode: 'queued' },
          ],
        },
      },
      {
        status: 200,
        body: { resourceType: 'Parameters', parameter: [{ name: 'candidates', resource: { resourceType: 'OperationOutcome', issue: [{ severity: 'warning', code: 'incomplete' }] } }] },
      },
    ]
    const transport = fakeTransport(() => replies.shift() ?? { status: 500 })
    const registries = registriesCommands(transport, '/fhir', NIN)
    const patient = toPatient({ ...emptyForm, family: 'Nakato', given: 'Grace', sex: 'female' }, NIN, 2026)

    const first = await registries.register(patient, false)
    expect(first.kind).toBe('candidates')
    if (first.kind !== 'candidates') throw new Error('candidates')
    expect(candidates(first.bundle)).toEqual([
      { id: 'p1', name: 'Grace Nakato', birthDate: '1990-04-02', sex: 'female', mrn: 'KAM013-C1-000001', grade: 'certain', score: 0.98 },
    ])
    expect(transport.sent[0]).toMatchObject({
      method: 'POST',
      path: '/fhir/Patient/$register',
      body: { resourceType: 'Parameters', parameter: [{ name: 'resource', resource: patient }, { name: 'confirmedNew', valueBoolean: false }] },
    })

    await expect(registries.register(patient, true)).resolves.toEqual({ kind: 'registered', id: 'p2', mrn: 'KAM013-C1-000002', outcome: 'queued' })
    expect(transport.sent[1]?.body).toMatchObject({ parameter: [{}, { name: 'confirmedNew', valueBoolean: true }] })

    const thin = await registries.register({ resourceType: 'Patient', gender: 'male' }, false)
    expect(thin.kind).toBe('insufficient')
    await expect(registries.register(patient, true)).rejects.toThrow('FHIR 500')
  })

  test('runtime.json can put FHIR beside the API, as central serves it', async () => {
    const urls: string[] = []
    const fetchImpl = (async (input: URL | RequestInfo) => {
      urls.push(String(input))
      return new Response(JSON.stringify({ resourceType: 'Bundle', type: 'searchset' }), { status: 200, headers: { 'content-type': 'application/fhir+json' } })
    }) as typeof fetch
    await httpPlatform('central', 'https://central.test/api', fetchImpl, undefined, 'https://central.test/fhir').fhir.search('Patient', {})
    await httpPlatform('central', 'https://central.test/api', fetchImpl).fhir.search('Patient', {})
    expect(urls.map((u) => new URL(u).pathname)).toEqual(['/fhir/Patient', '/api/fhir/Patient'])
  })
})
