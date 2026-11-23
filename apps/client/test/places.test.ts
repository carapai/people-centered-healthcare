import { describe, expect, test } from 'vitest'

import { type ImportPlan, type Place, PlacesError, applyImport, assign, changedFields, optionsQuery, placeLabel, planIsEmpty, previewImport, userQuery, withdraw } from '../src/features/places/api'
import { fakeTransport } from './fake'

const place = (code: string, level: Place['level'], parent: string | null, extra: Partial<Place> = {}): Place => ({ code, name: code, level, parent, active: true, ...extra })

const level = (l: Place['level']) => l

const empty: ImportPlan = { hierarchy: [], new: [], changed: [], closed: [], unchanged: 0, absent: [] }

describe('the NHFR import', () => {
  test('the file is previewed and applied as JSON', async () => {
    const plan: ImportPlan = { ...empty, new: [place('KAM013', 'facility', 'district-kampala')], unchanged: 2, absent: ['KAM099'] }
    const transport = fakeTransport((request) => ({ body: request.path.endsWith('/apply') ? { written: 1, plan } : plan }))
    const csv = 'nhfr_id,name,district,status\nKAM013,Kisenyi HC IV,district-kampala,open\n'
    expect(await previewImport(transport, csv)).toEqual(plan)
    expect((await applyImport(transport, csv)).written).toBe(1)
    expect(transport.sent.map((r) => [r.method, r.path, r.body])).toEqual([
      ['POST', '/places/nhfr/preview', { csv }],
      ['POST', '/places/nhfr/apply', { csv }],
    ])
  })

  test("a refused file carries the server's problems", async () => {
    const transport = fakeTransport(() => ({ status: 422, body: { detail: 'the NHFR file cannot be imported', problems: ['row 2: district-nowhere is not a district of the national core package'] } }))
    await expect(previewImport(transport, 'x')).rejects.toEqual(new PlacesError(422, 'the NHFR file cannot be imported', ['row 2: district-nowhere is not a district of the national core package']))
  })

  test('a plan that changes nothing is empty, whatever is unchanged or absent', () => {
    expect(planIsEmpty({ ...empty, unchanged: 7, absent: ['KAM099'] })).toBe(true)
    expect(planIsEmpty({ ...empty, hierarchy: [place('district-gulu', 'district', 'region-northern')] })).toBe(false)
  })

  test('a change lists the fields it changes', () => {
    const before = place('KAM013', 'facility', 'district-kampala', { name: 'Kisenyi', care_level: 'HC III', coordinates: { latitude: '0.31', longitude: '32.56' } })
    const after = { ...before, name: 'Kisenyi HC IV', care_level: 'HC IV', active: false }
    expect(changedFields({ before, after })).toEqual(['name', 'active', 'care_level'])
    expect(changedFields({ before, after: { ...before, coordinates: { latitude: '0.32', longitude: '32.56' } } })).toEqual(['coordinates'])
  })
})

describe('role assignment at a place', () => {
  test('the options, a user, an assignment and a withdrawal go to central', async () => {
    const options = { roles: ['district', 'clinician'], places: [place('central', 'national', null), place('district-kampala', 'district', 'region-central')] }
    const held = { username: 'e2e wakiso', account: 'a1', assignments: [] }
    const transport = fakeTransport((request) => {
      if (request.path === '/assignments/options') return { body: options }
      if (request.method === 'GET') return { body: held }
      if (request.method === 'DELETE') return { status: 204 }
      return { body: { roles: ['district'] } }
    })
    expect(await optionsQuery(transport).queryFn({})).toEqual(options)
    expect(await userQuery(transport, 'e2e wakiso').queryFn({})).toEqual(held)
    await assign(transport, 'e2e wakiso', 'district-kampala', ['district'])
    await withdraw(transport, 'e2e wakiso', 'district-kampala')
    expect(transport.sent.map((r) => [r.method, r.path, r.body])).toEqual([
      ['GET', '/assignments/options', undefined],
      ['GET', '/assignments/users/e2e%20wakiso', undefined],
      ['PUT', '/assignments/users/e2e%20wakiso/district-kampala', { roles: ['district'] }],
      ['DELETE', '/assignments/users/e2e%20wakiso/district-kampala', undefined],
    ])
  })

  test('refusals carry their status and explanation', async () => {
    const forbidden = fakeTransport(() => ({ status: 403, body: { detail: 'district-kampala is outside your area' } }))
    await expect(assign(forbidden, 'okello', 'district-kampala', ['district'])).rejects.toEqual(new PlacesError(403, 'district-kampala is outside your area'))
    const missing = fakeTransport(() => ({ status: 404, body: { detail: 'there is no user nobody' } }))
    await expect(userQuery(missing, 'nobody').queryFn({})).rejects.toEqual(new PlacesError(404, 'there is no user nobody'))
    await expect(withdraw(missing, 'okello', 'KAM013')).rejects.toBeInstanceOf(PlacesError)
  })

  test('a place is named with its level, a facility with its code', () => {
    expect(placeLabel(place('district-kampala', 'district', 'region-central', { name: 'Kampala' }), level)).toBe('Kampala (district)')
    expect(placeLabel(place('KAM013', 'facility', 'district-kampala', { name: 'Kisenyi HC IV' }), level)).toBe('Kisenyi HC IV (KAM013)')
  })
})
