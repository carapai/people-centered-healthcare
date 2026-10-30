import { QueryClient } from '@tanstack/react-query'
import { describe, expect, test } from 'vitest'

import { fhirClient } from '../src/platform/fhir'
import { connectQueryCache, fhirKeys, readQuery, searchQuery, vreadQuery, worklistKey } from '../src/platform/query'
import { memorySession } from '../src/platform/session'
import type { InvalidationHint } from '../src/platform/types'
import { fakeTransport } from './fake'

function seeded() {
  const client = new QueryClient()
  const keys = {
    patient1: fhirKeys.resource('Patient', 'p1'),
    patient2: fhirKeys.resource('Patient', 'p2'),
    patientSearch: fhirKeys.search('Patient', { name: 'x' }),
    patientHistory: fhirKeys.history({ type: 'Patient', id: 'p2' }),
    systemHistory: fhirKeys.history({}),
    observation: fhirKeys.resource('Observation', 'o1'),
    worklist: worklistKey('triage'),
  }
  for (const key of Object.values(keys)) client.setQueryData(key, {})
  const stale = () =>
    Object.entries(keys)
      .filter(([, key]) => client.getQueryState(key)?.isInvalidated)
      .map(([name]) => name)
  return { client, stale }
}

function harness() {
  const listeners: ((hint: InvalidationHint) => void)[] = []
  const events = { subscribe: (listener: (hint: InvalidationHint) => void) => (listeners.push(listener), () => {}) }
  return { session: memorySession(), events, emit: (hint: InvalidationHint) => listeners.forEach((listener) => listener(hint)) }
}

describe('connectQueryCache', () => {
  test('a resource hint stales the resource and its type’s searches and histories', () => {
    const { client, stale } = seeded()
    const platform = harness()
    connectQueryCache(platform, client)
    platform.emit({ resourceType: 'Patient', id: 'p1' })
    expect(stale()).toEqual(['patient1', 'patientSearch', 'patientHistory', 'systemHistory'])
  })

  test('a type hint stales the whole type; a worklist hint its worklist', () => {
    const { client, stale } = seeded()
    const platform = harness()
    connectQueryCache(platform, client)
    platform.emit({ resourceType: 'Patient' })
    platform.emit({ worklist: 'triage' })
    expect(stale()).toEqual(['patient1', 'patient2', 'patientSearch', 'patientHistory', 'worklist'])
  })

  test.each(['logout', 'lock'] as const)('%s clears the cache and the token', (reason) => {
    const { client } = seeded()
    const platform = harness()
    platform.session.setAccessToken('t')
    connectQueryCache(platform, client)
    platform.session.end(reason)
    expect(client.getQueryCache().getAll()).toHaveLength(0)
    expect(platform.session.accessToken()).toBeUndefined()
  })
})

test('query options fetch through the FhirClient', async () => {
  const transport = fakeTransport((request) => ({
    body: request.path.endsWith('_search') ? { resourceType: 'Bundle', type: 'searchset' } : { resourceType: 'Patient', id: 'p1' },
  }))
  const fhir = fhirClient(transport)
  const client = new QueryClient()
  expect(await client.fetchQuery(readQuery(fhir, 'Patient', 'p1'))).toMatchObject({ id: 'p1' })
  await client.fetchQuery(searchQuery(fhir, 'Patient', { name: 'x' }, 'POST'))
  expect(vreadQuery(fhir, 'Patient', 'p1', '2').staleTime).toBe(Infinity)
  expect(transport.sent.map((r) => r.method)).toEqual(['GET', 'POST'])
})
