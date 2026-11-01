import { describe, expect, test } from 'vitest'

import { CaregiverError, adolescentConsent, caregiversQuery, dependentsQuery, recordCaregiver, revokeCaregiver } from '../src/features/caregivers/api'
import { fakeTransport } from './fake'

const grant = {
  id: 'g1',
  caregiver: 'a1',
  patient: 'p1',
  authority: 'guardian',
  scopes: ['view-records'],
  expires: '2031-02-01T23:59:59Z',
  verified: 'birth-certificate',
  adolescent_consents: false,
  status: 'active',
}

describe('caregivers', () => {
  test("a patient's caregivers are listed, and refusals carry the server's reason", async () => {
    const transport = fakeTransport(() => ({ body: [grant] }))
    await expect(caregiversQuery(transport, 'p 1').queryFn({})).resolves.toEqual([grant])
    expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/caregivers?patient=p%201' })
    await expect(caregiversQuery(fakeTransport(() => ({ status: 403, body: { error: 'no' } })), 'p1').queryFn({})).rejects.toEqual(
      new CaregiverError(403, 'no'),
    )
  })

  test('staff record, revoke and record consent', async () => {
    const created = fakeTransport(() => ({ status: 201, body: grant }))
    const caregiver = {
      patient: 'p1',
      phone: '0772100003',
      relationship: 'MTH',
      authority: 'guardian',
      scopes: ['view-records'],
      expires: '2031-02-01',
      document: 'birth certificate',
    } as const
    await expect(recordCaregiver(created, { ...caregiver, scopes: [...caregiver.scopes] })).resolves.toEqual(grant)
    expect(created.sent[0]).toMatchObject({ method: 'POST', path: '/caregivers', body: { phone: '0772100003', scopes: ['view-records'] } })
    const refused = fakeTransport(() => ({ status: 422, body: { error: 'no portal account has this phone' } }))
    await expect(recordCaregiver(refused, { ...caregiver, scopes: [...caregiver.scopes] })).rejects.toMatchObject({ status: 422, reason: 'no portal account has this phone' })
    const ok = fakeTransport(() => ({ body: grant }))
    await revokeCaregiver(ok, 'g1')
    await adolescentConsent(ok, 'g1', true)
    expect(ok.sent.map((r) => [r.path, r.body])).toEqual([
      ['/caregivers/g1/revoke', undefined],
      ['/caregivers/g1/adolescent-consent', { consents: true }],
    ])
  })

  test('the portal lists the people a caregiver cares for', async () => {
    const transport = fakeTransport(() => ({ body: [{ ...grant, name: 'Amina Okot', confidential_withheld: true }] }))
    await expect(dependentsQuery(transport).queryFn({})).resolves.toMatchObject([{ name: 'Amina Okot', confidential_withheld: true }])
    expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/portal/dependents' })
  })
})
