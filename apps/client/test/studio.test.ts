import { ResponseError, type StudioPackage } from '@health/api-client'
import { createIntl } from 'react-intl'
import { describe, expect, test, vi } from 'vitest'

import { StudioError, needs, packageQuery, packagesQuery, studioKey, takeAction } from '../src/features/studio/api'
import { actionLabel, refusalText } from '../src/features/studio/wording'
import { httpPlatform } from '../src/platform/compose'
import { fakeTransport } from './fake'

// The Studio (#492): the client renders the server's actions and reasons, and holds no
// lifecycle rule of its own.

const authorRefusal = { code: 'author', message: 'You wrote this draft, so you cannot review or approve it', permission: null, stage: null, missing: null } as const

const row: StudioPackage = {
  id: 'ug.moh.anc@1.1.0',
  name: 'ug.moh.anc',
  title: 'ANC',
  kind: 'program',
  version: '1.1.0',
  previous: '1.0.0',
  bump: 'minor',
  stage: 'approval',
  authors: ['ann'],
  active: [{ scope: 'national', version: '1.0.0' }],
  actions: [
    { action: 'submit', allowed: false, refusal: { code: 'stage', message: 'Not possible while it is at the approval stage', permission: null, stage: 'approval', missing: null } },
    { action: 'approve', allowed: false, refusal: authorRefusal },
    { action: 'request-changes', allowed: false, refusal: authorRefusal },
  ],
  next: null,
}

const page = {
  package: row,
  changes: [{ change: 'changed', resourceType: 'Questionnaire', path: 'Questionnaire-anc-visit.json', url: 'http://health.go.ug/fhir/Questionnaire/anc-visit', before: '1.0.0', after: '1.1.0' }],
  checks: [{ check: 'validation', passed: true, problems: [], at: '2026-11-19T09:00:00Z', current: true }],
  approvalsRequired: 2,
  reviewer: 'rita',
  approvers: [],
  history: [{ step: 'created', by: 'ann', at: '2026-11-19T08:00:00Z', stage: 'draft', reason: null, problems: [] }],
}

describe('the Studio', () => {
  test('the package list is read and checked against central’s schema', async () => {
    const transport = fakeTransport(() => ({ body: { packages: [row] } }))
    const query = packagesQuery(transport)
    expect(query.queryKey).toEqual(studioKey)
    await expect(query.queryFn()).resolves.toEqual([row])
    expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/studio/packages' })
    // A body that is not the documented one is refused at the boundary.
    const wrong = fakeTransport(() => ({ body: { packages: [{ ...row, stage: 'thinking' }] } }))
    await expect(packagesQuery(wrong).queryFn()).rejects.toBeInstanceOf(ResponseError)
  })

  test('a refusal carries its status, its detail and the server’s reason', async () => {
    const refused = fakeTransport(() => ({ status: 403, body: { detail: 'the Studio needs a metadata permission', refusal: null } }))
    await expect(packagesQuery(refused).queryFn()).rejects.toEqual(new StudioError(403, 'the Studio needs a metadata permission', null))
    const conflict = fakeTransport(() => ({ status: 409, body: { detail: 'an author may not review or approve their own package', refusal: authorRefusal } }))
    const error = await takeAction(conflict, row.id, 'approve').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(StudioError)
    expect((error as StudioError).refusal).toEqual(authorRefusal)
  })

  test('a package is read by its id, and a step is posted with what it needs', async () => {
    const transport = fakeTransport(() => ({ body: page }))
    await expect(packageQuery(transport, row.id).queryFn()).resolves.toEqual(page)
    expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/studio/packages/ug.moh.anc%401.1.0' })
    await takeAction(transport, row.id, 'request-changes', { reason: 'Reword the pulse question' })
    expect(transport.sent[1]).toMatchObject({ method: 'POST', path: '/studio/packages/ug.moh.anc%401.1.0/request-changes', body: { reason: 'Reword the pulse question' } })
    expect([needs('request-changes'), needs('activate'), needs('approve')]).toEqual(['reason', 'scope', null])
  })

  test('the server’s reasons are worded, and one it has no words for is shown as sent', () => {
    const intl = createIntl({ locale: 'en', defaultLocale: 'en' })
    expect(refusalText(intl, authorRefusal)).toBe('You wrote this draft, so you cannot review or approve it')
    expect(refusalText(intl, { code: 'approvals-needed', message: '', missing: 1 })).toBe('1 more approval needed')
    expect(refusalText(intl, { code: 'approvals-needed', message: '', missing: 2 })).toBe('2 more approvals needed')
    expect(refusalText(intl, { code: 'permission', message: '', permission: 'metadata.publish' })).toBe('You need the metadata.publish permission')
    expect(refusalText(intl, { code: 'stage', message: '', stage: 'review' })).toBe('Not possible while it is at the review stage')
    expect(refusalText(intl, { code: 'other', message: 'who acted must be given' })).toBe('who acted must be given')
    expect(actionLabel(intl, 'approve')).toBe('Approve')
  })

  test('only central offers the Studio; an edge never does', () => {
    vi.stubGlobal('location', { origin: 'http://localhost' })
    expect(httpPlatform('central', '/api', fetch).capabilities.studio).toBe(true)
    expect(httpPlatform('edge', '/api', fetch).capabilities.studio).toBe(false)
    vi.unstubAllGlobals()
  })
})
