import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { studioMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button } from '../reception/styles'
import { StudioError, fileQuery, packageQuery, removeFile, saveFile, studioKey, type StudioFile, type StudioPackagePage } from './api'
import { refusalText } from './wording'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '6xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted' })
const editor = css({ minHeight: '28rem', width: '100%', fontFamily: 'mono', fontSize: 'sm', p: '2', borderRadius: 'md', border: '1px solid', borderColor: 'border' })
const alert = css({ color: 'danger.text' })

/**
 * One file of a package version in a plain JSON editor (studio.md § The authoring API, Editing;
 * #494). Each save is this one file, sent with the draft's revision as `If-Match`: when someone
 * else saved the draft first, the server refuses it (412) and keeps nothing, and the person
 * reloads instead of overwriting their change. A path the draft does not hold yet opens empty
 * and is added on its first save. Whether the person may edit is the server's answer.
 */
export function FileEditor({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  const file = useQuery({ ...fileQuery(platform.transport, id, path), retry: false })
  const missing = file.error instanceof StudioError && file.error.status === 404
  return (
    <section className={page}>
      <div>
        <Link to="/studio/$id" params={{ id }}>
          {intl.formatMessage(m.sdEditorBack)}
        </Link>
      </div>
      <h1 className={heading}>{intl.formatMessage(m.sdEditorHeading, { path, id })}</h1>
      {draft.data === undefined || (file.data === undefined && !missing) ? (
        draft.isError || (file.isError && !missing) ? (
          <p role="alert" className={alert}>
            {intl.formatMessage(m.sdFailed)}
          </p>
        ) : (
          <p role="status">{intl.formatMessage(m.sdEditorLoading)}</p>
        )
      ) : (
        <Editor
          key={`${file.data?.revision ?? 'new'}:${file.dataUpdatedAt}`}
          id={id}
          path={path}
          draft={draft.data}
          file={file.data ?? null}
          onReload={async () => {
            await Promise.all([draft.refetch(), file.refetch()])
          }}
        />
      )}
    </section>
  )
}

/** Pretty-prints JSON text, leaving anything else as it is. */
function pretty(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return text
  }
}

function Editor({ id, path, draft, file, onReload }: { id: string; path: string; draft: StudioPackagePage; file: StudioFile | null; onReload: () => Promise<void> }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const contentId = useId()
  const errorId = useId()
  const [text, setText] = useState(() => (file === null ? '{\n  "resourceType": ""\n}' : pretty(file.content)))
  // The revision this text was read at: the file's, or the page's for a new file. A save moves it on.
  const [revision, setRevision] = useState(file?.revision ?? draft.revision)
  const [removed, setRemoved] = useState(false)
  const notJson = (() => {
    try {
      JSON.parse(text)
      return false
    } catch {
      return true
    }
  })()
  const settle = async (next: StudioPackagePage) => {
    setRevision(next.revision)
    queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
    await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
  }
  const save = useMutation({
    mutationFn: () => saveFile(platform.transport, id, path, text, revision),
    onSuccess: settle,
  })
  const remove = useMutation({
    mutationFn: () => removeFile(platform.transport, id, path, revision),
    onSuccess: async (next) => {
      setRemoved(true)
      await settle(next)
    },
  })
  const failed = save.error ?? remove.error
  const stale = failed instanceof StudioError && failed.status === 412
  const reason =
    failed instanceof StudioError ? (failed.refusal === null || failed.refusal === undefined ? failed.detail : refusalText(intl, failed.refusal)) : failed === null ? '' : String(failed)
  const editable = draft.editable && !removed
  const required = path === 'package.json' || path === 'Library-manifest.json'
  return (
    <>
      {file === null && !removed ? <p className={muted}>{intl.formatMessage(m.sdEditorNew)}</p> : null}
      {editable ? null : (
        <p className={muted}>
          {draft.editRefusal === null || draft.editRefusal === undefined
            ? intl.formatMessage(m.sdEditorReadOnly)
            : intl.formatMessage(m.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}
        </p>
      )}
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        <label htmlFor={contentId}>{intl.formatMessage(m.sdEditorContent)}</label>
        <textarea
          id={contentId}
          className={editor}
          value={text}
          spellCheck={false}
          readOnly={!editable}
          aria-invalid={notJson}
          aria-describedby={notJson ? errorId : undefined}
          onChange={(e) => setText(e.target.value)}
        />
        {notJson ? (
          <span id={errorId} className={alert}>
            {intl.formatMessage(m.sdEditorNotJson)}
          </span>
        ) : null}
      </div>
      {editable ? (
        <div className={css({ display: 'flex', flexWrap: 'wrap', gap: '3' })}>
          <button type="button" className={button} disabled={notJson || save.isPending || stale} onClick={() => save.mutate()}>
            {intl.formatMessage(m.sdEditorSave)}
          </button>
          {file === null || required ? null : (
            <button type="button" className={button} disabled={remove.isPending || stale} onClick={() => remove.mutate()}>
              {intl.formatMessage(m.sdEditorRemove)}
            </button>
          )}
        </div>
      ) : null}
      {save.isSuccess && !stale ? <p role="status">{intl.formatMessage(m.sdEditorSaved)}</p> : null}
      {removed ? <p role="status">{intl.formatMessage(m.sdEditorRemoved)}</p> : null}
      {stale ? (
        <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
          <p className={alert}>{intl.formatMessage(m.sdEditorStale)}</p>
          <button type="button" className={button} onClick={() => void onReload()}>
            {intl.formatMessage(m.sdEditorReload)}
          </button>
        </div>
      ) : failed === null ? null : (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.sdEditorFailed, { reason })}
        </p>
      )}
    </>
  )
}
