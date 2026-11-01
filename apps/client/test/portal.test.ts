import { describe, expect, test, vi } from 'vitest'

import { PortalError, QUESTIONNAIRE, canSubmit, questionnaireResponse, registrationQuery, submitRegistration } from '../src/features/portal/api'
import { httpPlatform } from '../src/platform/compose'
import { fakeTransport } from './fake'

const answers = { given: ' Grace ', family: 'Nakato', birthDate: '1990-04-12', gender: 'female' } as const

describe('patient portal', () => {
  test('no registration yet is null, and refusals carry their status', async () => {
    const none = fakeTransport(() => ({ status: 404 }))
    await expect(registrationQuery(none).queryFn({})).resolves.toBeNull()
    expect(none.sent[0]).toMatchObject({ method: 'GET', path: '/portal/registration' })
    const mine = fakeTransport(() => ({ body: { status: 'review', patient: null, access: 'none' } }))
    await expect(registrationQuery(mine).queryFn({})).resolves.toEqual({ status: 'review', patient: null, access: 'none' })
    await expect(registrationQuery(fakeTransport(() => ({ status: 401 }))).queryFn({})).rejects.toEqual(new PortalError(401))
  })

  test('the answers are sent as the self-registration QuestionnaireResponse', async () => {
    expect(canSubmit(answers)).toBe(true)
    expect(canSubmit({ ...answers, gender: '' })).toBe(false)
    expect(questionnaireResponse(answers)).toMatchObject({
      resourceType: 'QuestionnaireResponse',
      questionnaire: QUESTIONNAIRE,
      item: [
        { linkId: 'given', answer: [{ valueString: 'Grace' }] },
        { linkId: 'family', answer: [{ valueString: 'Nakato' }] },
        { linkId: 'birthDate', answer: [{ valueDate: '1990-04-12' }] },
        { linkId: 'gender', answer: [{ valueCoding: { code: 'female' } }] },
      ],
    })
    const transport = fakeTransport(() => ({ status: 201, body: { status: 'completed', patient: 'p1', access: 'self-submitted' } }))
    await expect(submitRegistration(transport, answers)).resolves.toMatchObject({ access: 'self-submitted' })
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/portal/registration' })
    await expect(submitRegistration(fakeTransport(() => ({ status: 409 })), answers)).rejects.toEqual(new PortalError(409))
  })

  test('the portal has its own session, apart from staff', () => {
    vi.stubGlobal('location', { origin: 'http://localhost' })
    const auth = { issuer: '/auth', clientId: 'health-client', redirectPath: '/callback' }
    const platform = httpPlatform('central', '/api', fetch, auth, '/fhir', undefined, { clientId: 'health-portal', redirectPath: '/portal/callback' })
    expect(platform.portal?.clientId).toBe('health-portal')
    expect(platform.portal?.session).not.toBe(platform.session)
    expect(httpPlatform('edge', '/api', fetch, auth, '/fhir', undefined, { clientId: 'health-portal', redirectPath: '/portal/callback' }).portal).toBeUndefined()
    expect(httpPlatform('central', '/api', fetch, auth).portal).toBeUndefined()
    vi.unstubAllGlobals()
  })
})
