import { ResponseError, type StudioPackage } from '@health/api-client'
import { createIntl } from 'react-intl'
import { describe, expect, test, vi } from 'vitest'

import { StudioError, actionInput, fileQuery, needs, packageQuery, packagesQuery, removeFile, saveFile, setNote, setVersion, startDraft, studioKey, takeAction } from '../src/features/studio/api'
import { actionLabel, refusalText } from '../src/features/studio/wording'
import { httpPlatform } from '../src/platform/compose'
import { fakeTransport } from './fake'
import { studioMessages } from '../src/i18n/messages'

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
  holder: 'ann',
  offeredTo: null,
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
  changes: [{ change: 'changed', resourceType: 'Questionnaire', path: 'Questionnaire-anc-visit.json', url: 'http://health.go.ug/fhir/Questionnaire/anc-visit', before: '1.0.0', after: '1.1.0', elements: [{ at: 'item[bp].text', change: 'reworded', bump: 'patch', summary: 'Item bp: text reworded' }] }],
  checks: [{ check: 'validation', passed: true, problems: [], at: '2026-11-19T09:00:00Z', current: true }],
  approvalsRequired: 2,
  reviewer: 'rita',
  approvers: [],
  history: [{ step: 'created', by: 'ann', at: '2026-11-19T08:00:00Z', stage: 'draft', reason: null, person: null, problems: [], previous: null }],
  revision: 3,
  editable: false,
  editRefusal: { code: 'stage', message: 'Not possible while it is at the approval stage', permission: null, stage: 'approval', missing: null },
  files: [{ path: 'Questionnaire-anc-visit.json', resourceType: 'Questionnaire', url: 'http://health.go.ug/fhir/Questionnaire/anc-visit', version: '1.1.0' }],
  note: 'Units on the BP question',
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
    expect([needs('request-changes'), needs('activate'), needs('approve')]).toEqual([['reason'], ['scope'], []])
  })

  // #493: a handover names who by username; a reassignment also says why; withdrawal needs nothing.
  test('handing over, reassigning and withdrawing send what each needs', async () => {
    const fields = { reason: 'ann is on leave', scope: 'national', to: ' e2e-coauthor ' }
    expect(actionInput('hand-over', fields)).toEqual({ to: 'e2e-coauthor' })
    expect(actionInput('reassign', fields)).toEqual({ to: 'e2e-coauthor', reason: 'ann is on leave' })
    expect(actionInput('accept-handover', fields)).toEqual({})
    expect(actionInput('withdraw', fields)).toEqual({})
    expect(actionInput('activate', fields)).toEqual({ scope: 'national' })
    const withdrawn = { ...page, package: { ...row, id: 'ug.moh.anc@1.1.0~withdrawn-20261120090000', stage: 'withdrawn' } }
    const transport = fakeTransport(() => ({ body: withdrawn }))
    const taken = await takeAction(transport, row.id, 'withdraw')
    expect(taken.package.id).toBe('ug.moh.anc@1.1.0~withdrawn-20261120090000')
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/studio/packages/ug.moh.anc%401.1.0/withdraw' })
    await takeAction(transport, row.id, 'hand-over', actionInput('hand-over', fields))
    expect(transport.sent[1]).toMatchObject({ path: '/studio/packages/ug.moh.anc%401.1.0/hand-over', body: { to: 'e2e-coauthor' } })
  })

  // #494: each save is one file, sent with the revision it was read at; a stale one is a 412.
  test('a file is saved with If-Match, and a stale save is refused with 412', async () => {
    const transport = fakeTransport((request) =>
      request.method === 'GET'
        ? { body: { path: 'tests/visit.json', content: '{}', revision: 3 } }
        : request.headers?.['if-match'] === '"3"'
          ? { body: { ...page, revision: 4 } }
          : { status: 412, body: { detail: 'the draft changed since you read it: reload it before saving', refusal: null } },
    )
    const file = await fileQuery(transport, row.id, 'tests/visit.json').queryFn()
    expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/studio/packages/ug.moh.anc%401.1.0/files/tests/visit.json' })
    const saved = await saveFile(transport, row.id, 'tests/visit.json', '{"a":1}', file.revision)
    expect(saved.revision).toBe(4)
    expect(transport.sent[1]).toMatchObject({ method: 'PUT', headers: { 'if-match': '"3"' }, body: { content: '{"a":1}' } })
    // The other tab, still at revision 2, is refused and told to reload.
    const stale = await saveFile(transport, row.id, 'tests/visit.json', '{"a":2}', 2).catch((e: unknown) => e)
    expect(stale).toBeInstanceOf(StudioError)
    expect((stale as StudioError).status).toBe(412)
    await expect(removeFile(transport, row.id, 'tests/visit.json', 3)).resolves.toMatchObject({ revision: 4 })
    expect(transport.sent.at(-1)).toMatchObject({ method: 'DELETE', headers: { 'if-match': '"3"' } })
    await setVersion(transport, row.id, '1.2.0', 3)
    expect(transport.sent.at(-1)).toMatchObject({ method: 'PUT', path: '/studio/packages/ug.moh.anc%401.1.0/version', body: { version: '1.2.0' } })
  })

  test('a draft starts as a copy of a published version, answered with 201', async () => {
    const transport = fakeTransport(() => ({ status: 201, body: { ...page, package: { ...row, id: 'ug.moh.anc@1.2.0-draft', version: '1.2.0-draft', stage: 'draft' } } }))
    const started = await startDraft(transport, { copyOf: row.id })
    expect(started.package.version).toBe('1.2.0-draft')
    expect(transport.sent[0]).toMatchObject({ method: 'POST', path: '/studio/drafts', body: { copyOf: row.id } })
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
    expect(actionLabel(intl, 'check')).toBe('Check now')
    expect(refusalText(intl, { code: 'not-holder', message: '' })).toBe("Only the draft's holder may do that")
    expect(refusalText(intl, { code: 'not-offered', message: '' })).toBe('No handover of this draft is waiting for you')
    expect(actionLabel(intl, 'accept-handover')).toBe('Accept the handover')
    expect(actionLabel(intl, 'withdraw')).toBe('Withdraw')
  })

  test('only central offers the Studio; an edge never does', () => {
    vi.stubGlobal('location', { origin: 'http://localhost' })
    expect(httpPlatform('central', '/api', fetch).capabilities.studio).toBe(true)
    expect(httpPlatform('edge', '/api', fetch).capabilities.studio).toBe(false)
    vi.unstubAllGlobals()
  })

  // #495: each changed artefact carries its changed elements, and the holder's note is kept
  // under the same If-Match as a save.
  test('what changed carries each changed element, and the change note is saved with If-Match', async () => {
    const transport = fakeTransport(() => ({ body: page }))
    const read = await packageQuery(transport, row.id).queryFn()
    expect(read.changes[0]?.elements).toEqual([{ at: 'item[bp].text', change: 'reworded', bump: 'patch', summary: 'Item bp: text reworded' }])
    expect(read.note).toBe('Units on the BP question')
    await setNote(transport, row.id, 'Units on the BP question', 3)
    expect(transport.sent[1]).toMatchObject({ method: 'PUT', path: '/studio/packages/ug.moh.anc%401.1.0/note', headers: { 'if-match': '"3"' }, body: { note: 'Units on the BP question' } })
    const intl = createIntl({ locale: 'en', messages: {} })
    expect(intl.formatMessage(studioMessages.sdElementChange, { summary: 'Item bp: text reworded', bump: 'patch' })).toBe('Item bp: text reworded (needs a patch)')
    const wrong = fakeTransport(() => ({ body: { ...page, changes: [{ ...page.changes[0], elements: [{ at: 'x', change: 'moved', bump: 'patch', summary: 'x' }] }] } }))
    await expect(packageQuery(wrong, row.id).queryFn()).rejects.toBeInstanceOf(ResponseError)
  })
})
