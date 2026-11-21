import { css, cx } from '@health/design-system/css'
import type { StudioPackagePage } from '@health/api-client'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { studioMessages, terminologyMessages as m } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button, control } from '../../reception/styles'
import { StudioError, packageQuery, studioKey } from '../api'
import { refusalText } from '../wording'
import { codeSystemChangeQuery, editConcept, terminologyKey, type StudioConcept, type StudioConceptEditInput } from './api'
import { addCode, changeCode, conceptActions, dropCode, type ConceptFields } from './model'

// Editing a local CodeSystem's concepts in the terminology browser (studio.md § Terminology,
// Editing a local CodeSystem; #524). Each edit is saved on its own, with the draft's revision as
// `If-Match`; the server keeps a published code fixed (retired, never deleted, renamed or moved),
// gives the CodeSystem its next version, and works out what the change reaches.

const panel = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.canvas', p: '3', display: 'flex', flexDirection: 'column', gap: '2', minWidth: '0' })
const field = css({ display: 'flex', flexDirection: 'column', gap: '1' })
const input = cx(control, css({ width: '100%' }))
const minor = css({ fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '1', listStyle: 'none', p: '0', m: '0' })
const actions = css({ display: 'flex', flexWrap: 'wrap', gap: '2', alignItems: 'center' })
const secondary = css({
  minHeight: 'control',
  px: '3',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})

/** What editing needs: the CodeSystem's file and the draft's revision, kept by the browser. */
export type Authoring = { path: string; revision: number; onSaved: (page: StudioPackagePage) => void; onReload: () => Promise<void> }

const reasonOf = (intl: ReturnType<typeof useIntl>, error: unknown) =>
  error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail : refusalText(intl, error.refusal)) : error === null || error === undefined ? '' : String(error)

/** Sends one edit, and on success hands the page back and refreshes what the browser read. */
function useEdit(id: string, authoring: Authoring) {
  const platform = usePlatform()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (edit: StudioConceptEditInput) => editConcept(platform.transport, id, edit, authoring.revision),
    onSuccess: async (page) => {
      authoring.onSaved(page)
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, page)
      await queryClient.invalidateQueries({ queryKey: terminologyKey(id) })
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
}

function Outcome({ edit, onReload }: { edit: ReturnType<typeof useEdit>; onReload: () => Promise<void> }) {
  const intl = useIntl()
  if (edit.error instanceof StudioError && edit.error.status === 412) {
    return (
      <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
        <p className={alert}>{intl.formatMessage(m.tbStale)}</p>
        <button type="button" className={button} onClick={() => void onReload()}>
          {intl.formatMessage(m.tbReload)}
        </button>
      </div>
    )
  }
  if (edit.isError) {
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.ceFailed, { reason: reasonOf(intl, edit.error) })}
      </p>
    )
  }
  return edit.isSuccess ? <p role="status">{intl.formatMessage(m.ceSaved)}</p> : null
}

function Fields({ fields, onChange, fixed }: { fields: ConceptFields; onChange: (next: ConceptFields) => void; fixed: boolean }) {
  const intl = useIntl()
  const ids = { code: useId(), fixed: useId(), display: useId(), definition: useId(), parent: useId(), parentHint: useId(), note: useId(), noteHint: useId() }
  const set = (name: keyof ConceptFields) => (e: { target: { value: string } }) => onChange({ ...fields, [name]: e.target.value })
  return (
    <>
      <div className={field}>
        <label htmlFor={ids.code}>{intl.formatMessage(m.ceCode)}</label>
        <input id={ids.code} className={input} value={fields.code} readOnly={fixed} aria-describedby={fixed ? ids.fixed : undefined} required onChange={set('code')} />
        {fixed ? (
          <span id={ids.fixed} className={muted}>
            {intl.formatMessage(m.ceCodeFixed)}
          </span>
        ) : null}
      </div>
      <div className={field}>
        <label htmlFor={ids.display}>{intl.formatMessage(m.ceDisplay)}</label>
        <input id={ids.display} className={input} value={fields.display} onChange={set('display')} />
      </div>
      <div className={field}>
        <label htmlFor={ids.definition}>{intl.formatMessage(m.ceDefinition)}</label>
        <textarea id={ids.definition} className={input} rows={3} value={fields.definition} onChange={set('definition')} />
      </div>
      <div className={field}>
        <label htmlFor={ids.parent}>{intl.formatMessage(m.ceParent)}</label>
        <input id={ids.parent} className={input} value={fields.parent} readOnly={fixed} aria-describedby={ids.parentHint} onChange={set('parent')} />
        <span id={ids.parentHint} className={muted}>
          {intl.formatMessage(m.ceParentHint)}
        </span>
      </div>
      <div className={field}>
        <label htmlFor={ids.note}>{intl.formatMessage(m.ceNote)}</label>
        <input id={ids.note} className={input} value={fields.note} aria-describedby={ids.noteHint} onChange={set('note')} />
        <span id={ids.noteHint} className={muted}>
          {intl.formatMessage(m.ceNoteHint)}
        </span>
      </div>
    </>
  )
}

/** The form for a new code, at the top level or under `parent`. */
export function AddCode({ id, authoring, parent, onAdded, onCancel }: { id: string; authoring: Authoring; parent: string | undefined; onAdded: (code: string) => void; onCancel: () => void }) {
  const intl = useIntl()
  const headingId = useId()
  const [fields, setFields] = useState<ConceptFields>({ code: '', display: '', definition: '', parent: parent ?? '', note: '' })
  const edit = useEdit(id, authoring)
  return (
    <form
      aria-labelledby={headingId}
      className={panel}
      onSubmit={(e) => {
        e.preventDefault()
        const sent = addCode(authoring.path, fields)
        edit.mutate(sent, { onSuccess: () => onAdded(sent.code) })
      }}
    >
      <h3 id={headingId} className={minor}>
        {parent === undefined ? intl.formatMessage(m.ceAddHeading) : intl.formatMessage(m.ceAddUnderHeading, { code: parent })}
      </h3>
      <Fields fields={fields} onChange={setFields} fixed={false} />
      <div className={actions}>
        <button type="submit" className={button} disabled={edit.isPending || fields.code.trim() === ''}>
          {intl.formatMessage(m.ceAdd)}
        </button>
        <button type="button" className={secondary} onClick={onCancel}>
          {intl.formatMessage(m.ceCancel)}
        </button>
      </div>
      <Outcome edit={edit} onReload={authoring.onReload} />
    </form>
  )
}

/** The concept panel as a form: a new code is renamed, moved or deleted; a published one keeps
 * its code and parent and is retired. */
export function EditCode({ id, authoring, concept, onRenamed }: { id: string; authoring: Authoring; concept: StudioConcept; onRenamed: (code: string | null) => void }) {
  const intl = useIntl()
  const headingId = useId()
  const offered = conceptActions(concept)
  const [fields, setFields] = useState<ConceptFields>({ code: concept.code, display: concept.display ?? '', definition: concept.definition ?? '', parent: concept.parents[0] ?? '', note: '' })
  const edit = useEdit(id, authoring)
  return (
    <form
      aria-labelledby={headingId}
      className={panel}
      onSubmit={(e) => {
        e.preventDefault()
        const sent = changeCode(authoring.path, concept.code, fields)
        edit.mutate(sent, { onSuccess: () => (sent.newCode === undefined || sent.newCode === null ? undefined : onRenamed(sent.newCode)) })
      }}
    >
      <h3 id={headingId} className={minor}>
        {intl.formatMessage(m.ceEditHeading, { code: concept.code })}
      </h3>
      {concept.inactive ? <p className={muted}>{intl.formatMessage(m.ceRetiredHint)}</p> : null}
      <Fields fields={fields} onChange={setFields} fixed={!offered.rename} />
      <div className={actions}>
        <button type="submit" className={button} disabled={edit.isPending || fields.code.trim() === ''}>
          {intl.formatMessage(m.ceSave)}
        </button>
        {offered.remove ? (
          <button type="button" className={secondary} disabled={edit.isPending} onClick={() => edit.mutate(dropCode(authoring.path, 'delete', concept.code, fields.note), { onSuccess: () => onRenamed(null) })}>
            {intl.formatMessage(m.ceDelete, { code: concept.code })}
          </button>
        ) : null}
        {offered.retire ? (
          <button type="button" className={secondary} disabled={edit.isPending} onClick={() => edit.mutate(dropCode(authoring.path, 'retire', concept.code, fields.note))}>
            {intl.formatMessage(m.ceRetire, { code: concept.code })}
          </button>
        ) : null}
      </div>
      <Outcome edit={edit} onReload={authoring.onReload} />
    </form>
  )
}

const codes = (found: { code: string }[]) => found.map((c) => c.code).join(', ')

/** Beside the tree: the next version and why, and what the change reaches. */
export function CodeSystemChange({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const change = useQuery({ ...codeSystemChangeQuery(platform.transport, id, path), retry: false, placeholderData: (previous) => previous })
  const data = change.data
  if (data === undefined) {
    return change.isError ? (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.tbFailed, { reason: reasonOf(intl, change.error) })}
      </p>
    ) : (
      <p role="status">{intl.formatMessage(m.tbLoading)}</p>
    )
  }
  return (
    <>
      <section aria-labelledby="ce-version" className={panel}>
        <h3 id="ce-version" className={minor}>
          {intl.formatMessage(m.ceVersion)}
        </h3>
        {data.published === null || data.published === undefined ? (
          <p>{intl.formatMessage(m.ceVersionFirst, { version: data.version ?? '' })}</p>
        ) : data.bump === 'none' ? (
          <p>{intl.formatMessage(m.ceVersionSame, { published: data.published })}</p>
        ) : (
          <>
            <p>{intl.formatMessage(m.ceVersionNext, { next: data.next ?? '', published: data.published, bump: data.bump })}</p>
            <ul className={list}>
              {data.changes.map((c) => (
                <li key={c.at}>
                  {intl.formatMessage(studioMessages.sdElementChange, { summary: c.summary, bump: c.bump })} <code className={muted}>{c.at}</code>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
      <section aria-labelledby="ce-reaches" className={panel}>
        <h3 id="ce-reaches" className={minor}>
          {intl.formatMessage(m.ceReaches)}
        </h3>
        {data.reaches.length === 0 ? (
          <p className={muted}>{intl.formatMessage(m.ceReachesNone)}</p>
        ) : (
          <ul className={list}>
            {data.reaches.map((r) => (
              <li key={`${r.valueSet.package}:${r.valueSet.path}`} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
                <span className={minor}>{intl.formatMessage(m.ceReachValueSet, { title: r.valueSet.title ?? r.valueSet.url, package: r.valueSet.package })}</span>
                {r.gained.length === 0 && r.lost.length === 0 ? <span className={muted}>{intl.formatMessage(m.ceReachSame)}</span> : null}
                {r.gained.length === 0 ? null : <span>{intl.formatMessage(m.ceGains, { codes: codes(r.gained) })}</span>}
                {r.lost.length === 0 ? null : (
                  <span className={alert}>
                    <span aria-hidden="true">⚠ </span>
                    {intl.formatMessage(m.ceLoses, { codes: codes(r.lost) })} <span className={muted}>{intl.formatMessage(m.ceLosesHint)}</span>
                  </span>
                )}
                {r.boundBy.length === 0 ? (
                  <span className={muted}>{intl.formatMessage(m.ceReachUnbound)}</span>
                ) : (
                  r.boundBy.map((b) => (
                    <span key={`${b.package}:${b.path}:${b.linkId}`} className={muted}>
                      {intl.formatMessage(m.ceReachBinding, { form: b.title ?? b.questionnaire ?? b.path, linkId: b.linkId })}
                    </span>
                  ))
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  )
}
