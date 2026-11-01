import { describe, expect, test } from 'vitest'

import { ReviewError, compare, decide, decideRegistration, reviewQueue, submitted } from '../src/features/registry-review/api'
import { worklistKey } from '../src/platform'
import { fakeTransport } from './fake'

const NIN = 'urn:health:nin'

describe('registry review', () => {
  test('the queue is keyed by its worklist and refused lists carry their status', async () => {
    const ok = fakeTransport(() => ({ body: [] }))
    const query = reviewQueue(ok)
    expect(query.queryKey).toEqual(worklistKey('registry-reviews'))
    await expect(query.queryFn({})).resolves.toEqual([])
    expect(ok.sent[0]).toMatchObject({ method: 'GET', path: '/registry/reviews' })
    const refused = fakeTransport(() => ({ status: 403 }))
    await expect(reviewQueue(refused).queryFn({})).rejects.toEqual(new ReviewError(403))
  })

  test('a decision is posted to its item', async () => {
    const transport = fakeTransport(() => ({ status: 204 }))
    await decide(transport, 'r1', 'different-people')
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/registry/reviews/r1', body: { decision: 'different-people' } })
    await expect(decide(fakeTransport(() => ({ status: 422 })), 'r1', 'same-person')).rejects.toEqual(new ReviewError(422))
  })

  test('a potential match is compared field by field', () => {
    const rows = compare(
      {
        resourceType: 'Patient',
        name: [{ family: 'Nakato', given: ['Grace'] }],
        birthDate: '1990-04-02',
        identifier: [{ system: NIN, value: 'CF1' }],
        telecom: [{ system: 'phone', value: '0772000001' }],
      },
      {
        resourceType: 'Patient',
        name: [{ family: 'nakato', given: ['Grace', 'Mary'] }],
        birthDate: '1990-04-02',
        identifier: [
          { system: NIN, value: 'CF1' },
          { type: { coding: [{ code: 'MR' }] }, value: 'KAM013-C1-000001' },
        ],
      },
      NIN,
    )
    const byField = Object.fromEntries(rows.map((r) => [r.field, r]))
    expect(byField['family']?.agrees).toBe(true)
    expect(byField['given']?.agrees).toBe(false)
    expect(byField['birthDate']?.agrees).toBe(true)
    expect(byField['nin']?.agrees).toBe(true)
    expect(byField['phone']).toEqual({ field: 'phone', source: '0772000001', candidate: '', agrees: false })
    expect(byField['mrn']?.candidate).toBe('KAM013-C1-000001')
  })

  test('a portal registration is decided with the candidate and the document checked', async () => {
    const transport = fakeTransport(() => ({ status: 204 }))
    await decideRegistration(transport, 'task-1', { decision: 'link-patient', patient: 'p1', document: 'national-id' })
    expect(transport.sent[0]).toMatchObject({
      method: 'POST',
      path: '/registry/reviews/task-1',
      body: { decision: 'link-patient', patient: 'p1', document: 'national-id' },
    })
    await decideRegistration(transport, 'task-1', { decision: 'create-patient' })
    expect(transport.sent[1]).toMatchObject({ body: { decision: 'create-patient' } })
    await expect(decideRegistration(fakeTransport(() => ({ status: 409 })), 'task-1', { decision: 'create-patient' })).rejects.toEqual(
      new ReviewError(409),
    )
  })

  test("a portal registration's answers become the patient compared with the candidates", () => {
    expect(submitted(null)).toBeNull()
    const patient = submitted({
      resourceType: 'QuestionnaireResponse',
      questionnaire: 'urn:health:questionnaire/self-registration',
      status: 'completed',
      item: [
        { linkId: 'given', answer: [{ valueString: 'Grace' }] },
        { linkId: 'family', answer: [{ valueString: 'Nakato' }] },
        { linkId: 'birthDate', answer: [{ valueDate: '1990-04-12' }] },
        { linkId: 'gender', answer: [{ valueCoding: { code: 'female' } }] },
      ],
    })
    expect(patient).toEqual({
      resourceType: 'Patient',
      name: [{ family: 'Nakato', given: ['Grace'] }],
      birthDate: '1990-04-12',
      gender: 'female',
    })
  })
})
