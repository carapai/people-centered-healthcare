import type { OperationOutcome } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import { FhirError, fhirClient } from '../src/platform/fhir'
import { fakeTransport } from './fake'

const outcome: OperationOutcome = {
  resourceType: 'OperationOutcome',
  issue: [{ severity: 'error', code: 'not-found', diagnostics: 'Patient Grace Okello' }],
}

describe('fhirClient', () => {
  test('paths are under the FHIR base, with ids encoded', async () => {
    const transport = fakeTransport(() => ({ body: { resourceType: 'Patient', id: 'a/b' } }))
    const fhir = fhirClient(transport, '/api/fhir')
    await fhir.read('Patient', 'a/b')
    await fhir.vread('Patient', 'a', '7').catch(() => {})
    await fhir.history({}).catch(() => {})
    await fhir.history({ type: 'Patient' }).catch(() => {})
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual([
      'GET /api/fhir/Patient/a%2Fb',
      'GET /api/fhir/Patient/a/_history/7',
      'GET /api/fhir/_history',
      'GET /api/fhir/Patient/_history',
    ])
  })

  test('an error carries the OperationOutcome; its message quotes no diagnostics', async () => {
    const fhir = fhirClient(fakeTransport(() => ({ status: 404, body: outcome })))
    const error = (await fhir.read('Patient', 'x').catch((caught: unknown) => caught)) as FhirError
    expect(error).toBeInstanceOf(FhirError)
    expect(error).toMatchObject({ status: 404, outcome })
    expect(error.message).toBe('FHIR 404: not-found')
  })

  test('a 2xx body of another type is an error', async () => {
    const fhir = fhirClient(fakeTransport(() => ({ body: { resourceType: 'Observation' } })))
    await expect(fhir.read('Patient', 'x')).rejects.toMatchObject({ status: 200, outcome: undefined })
  })

  test('search over POST sends the parameters as a form', async () => {
    const transport = fakeTransport(() => ({ body: { resourceType: 'Bundle', type: 'searchset' } }))
    await fhirClient(transport).search('Patient', { name: 'x' }, { via: 'POST' })
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/fhir/Patient/_search', form: { name: 'x' } })
    expect(transport.sent[0]?.query).toBeUndefined()
  })

  test('$match posts Parameters with the Patient and options', async () => {
    const transport = fakeTransport(() => ({ body: { resourceType: 'Bundle', type: 'searchset' } }))
    await fhirClient(transport).match({ resourceType: 'Patient' }, { onlyCertainMatches: true, count: 3 })
    const { path, body, headers } = transport.sent[0] ?? {}
    expect(path).toBe('/fhir/Patient/$match')
    expect(headers?.['content-type']).toBe('application/fhir+json')
    expect(JSON.parse(JSON.stringify(body, (_, v: unknown) => (typeof v === 'object' && v !== null && 'isLosslessNumber' in v ? Number(String(v)) : v)))).toEqual({
      resourceType: 'Parameters',
      parameter: [
        { name: 'resource', resource: { resourceType: 'Patient' } },
        { name: 'onlyCertainMatches', valueBoolean: true },
        { name: 'count', valueInteger: 3 },
      ],
    })
  })

  test('$extract returns the Bundle, bare or inside Parameters', async () => {
    const bundle = { resourceType: 'Bundle', type: 'transaction' }
    for (const reply of [bundle, { resourceType: 'Parameters', parameter: [{ name: 'return', resource: bundle }] }]) {
      const transport = fakeTransport(() => ({ body: reply }))
      expect(await fhirClient(transport).extract({ resourceType: 'QuestionnaireResponse', questionnaire: 'Questionnaire/q', status: 'completed' })).toEqual(bundle)
      expect(transport.sent[0]?.path).toBe('/fhir/QuestionnaireResponse/$extract')
    }
  })

  test('$populate returns the response output', async () => {
    const response = { resourceType: 'QuestionnaireResponse', status: 'in-progress' }
    const transport = fakeTransport(() => ({ body: { resourceType: 'Parameters', parameter: [{ name: 'response', resource: response }] } }))
    expect(await fhirClient(transport).populate({ resourceType: 'Parameters' })).toEqual(response)
    expect(transport.sent[0]?.path).toBe('/fhir/Questionnaire/$populate')
  })
})
