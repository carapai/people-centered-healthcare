import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { indicatorMessages as im, programMessages as pm, studioMessages as m, terminologyMessages as tm, workflowMessages as wm } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button, control, field } from '../reception/styles'
import { StudioError, actionInput, addComment, needs, packageQuery, setNote, setVersion, startDraft, studioKey, takeAction, type StudioActionName, type StudioPackagePage } from './api'
import { actionLabel, checkLabel, refusalText, stageLabel } from './wording'
import { useWorkflowFiles } from './workflowFiles'

const page = css({ display: 'flex', flexDirection: 'column', gap: '6', maxWidth: '5xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '2', listStyle: 'disc', pl: '5' })
const muted = css({ color: 'fg.muted' })

/**
 * One package version read from top to bottom (studio.md § The package workspace; #492): what
 * changed, is it safe, who must act, and the discussion. The actions are the server's list, in
 * its order; one the person may not take stays on the page, disabled, with the server's reason.
 * Its files open one by one in the file editor (#494); in a draft the person may change the
 * version, and a published version can be copied into the next version's draft. Its Draft
 * Holder (#493) is named under the title, with any handover waiting to be accepted.
 */
export function PackagePage({ id }: { id: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  return (
    <section className={page}>
      <div>
        <Link to="/studio/packages">{intl.formatMessage(m.sdBack)}</Link>
      </div>
      {draft.data === undefined ? (
        draft.isPending ? (
          <p role="status">{intl.formatMessage(m.sdLoading)}</p>
        ) : (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(draft.error instanceof StudioError && draft.error.status === 403 ? m.sdRefused : m.sdFailed)}
          </p>
        )
      ) : (
        <Draft id={id} draft={draft.data} />
      )}
    </section>
  )
}

function Draft({ id, draft }: { id: string; draft: StudioPackagePage }) {
  const intl = useIntl()
  const workflows = useWorkflowFiles(id, draft)
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  const summary = draft.package
  const failing = draft.checks.filter((c) => !c.passed).length
  return (
    <>
      <header>
        <h1 className={heading}>{intl.formatMessage(m.sdPackageHeading, { title: summary.title ?? summary.name, version: summary.version })}</h1>
        <p className={muted}>
          {intl.formatMessage(m.sdSummary, { name: summary.name, kind: summary.kind, stage: stageLabel(intl, summary.stage), authors: summary.authors.join(', ') })}
        </p>
        {summary.stage === 'published' ? null : summary.stage === 'withdrawn' ? (
          <p>{intl.formatMessage(m.sdWithdrawn)}</p>
        ) : (
          <p>
            {summary.offeredTo === null || summary.offeredTo === undefined
              ? intl.formatMessage(m.sdHolder, { holder: summary.holder })
              : intl.formatMessage(m.sdOffered, { holder: summary.holder, to: summary.offeredTo })}
          </p>
        )}
      </header>

      <section aria-labelledby="studio-changed">
        <h2 id="studio-changed" className={subheading}>
          {intl.formatMessage(m.sdChanged)}
        </h2>
        <p className={muted}>
          {summary.previous === null || summary.previous === undefined ? intl.formatMessage(m.sdChangedFirst) : intl.formatMessage(m.sdChangedAgainst, { version: summary.previous })}
        </p>
        <ChangeNote id={id} draft={draft} />
        {draft.changes.length === 0 ? (
          <p>{intl.formatMessage(m.sdNoChanges)}</p>
        ) : (
          <ul className={list}>
            {draft.changes.map((change) => (
              <li key={`${change.change}:${change.path}`}>
                {change.change === 'added'
                  ? intl.formatMessage(m.sdChangeAdded, { type: change.resourceType, path: change.path, version: change.after ?? '' })
                  : change.change === 'removed'
                    ? intl.formatMessage(m.sdChangeRemoved, { type: change.resourceType, path: change.path, version: change.before ?? '' })
                    : intl.formatMessage(m.sdChangeChanged, { type: change.resourceType, path: change.path, before: change.before ?? '', after: change.after ?? '' })}
                {change.resourceType === 'Questionnaire' && change.change !== 'removed' ? (
                  <>
                    {' · '}
                    <DesignerLink id={id} path={change.path} />
                  </>
                ) : null}
                {isTerminology(change.resourceType) && change.change !== 'removed' ? (
                  <>
                    {' · '}
                    <TerminologyLink id={id} path={change.path} />
                  </>
                ) : null}
                {workflows.has(change.path) && change.change !== 'removed' ? (
                  <>
                    {' · '}
                    <WorkflowLink id={id} path={change.path} />
                  </>
                ) : null}
                {isProgram(draft, change.resourceType) && change.change !== 'removed' ? (
                  <>
                    {' · '}
                    <ProgramLink id={id} path={change.path} />
                  </>
                ) : null}
                {change.resourceType === 'Measure' && change.change !== 'removed' ? (
                  <>
                    {' · '}
                    <IndicatorLink id={id} path={change.path} />
                  </>
                ) : null}
                {change.elements.length === 0 ? null : (
                  <ul className={list}>
                    {change.elements.map((element) => (
                      <li key={`${element.at}:${element.change}`}>
                        {intl.formatMessage(m.sdElementChange, { summary: element.summary, bump: element.bump })}
                        {element.at === '' ? null : (
                          <>
                            {' · '}
                            <code className={muted}>{element.at}</code>
                          </>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <Files id={id} draft={draft} workflows={workflows} />

      <section aria-labelledby="studio-safe">
        <h2 id="studio-safe" className={subheading}>
          {intl.formatMessage(m.sdSafe, { failing })}
        </h2>
        {draft.checks.length === 0 ? (
          <p>{intl.formatMessage(m.sdNotChecked)}</p>
        ) : (
          <ul className={list}>
            {draft.checks.map((check) => (
              <li key={check.check}>
                {intl.formatMessage(check.passed ? m.sdCheckPassed : m.sdCheckFailed, { check: checkLabel(intl, check.check), at: time(check.at) })}
                {check.current ? null : <span className={muted}> {intl.formatMessage(m.sdCheckStale)}</span>}
                {check.problems.length === 0 ? null : (
                  <ul className={list}>
                    {check.problems.map((problem) => (
                      <li key={problem}>{problem}</li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-labelledby="studio-who" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <h2 id="studio-who" className={subheading}>
          {intl.formatMessage(m.sdWhoActs)}
        </h2>
        <ul className={list}>
          <li>{draft.reviewer === null || draft.reviewer === undefined ? intl.formatMessage(m.sdNoReviewer) : intl.formatMessage(m.sdReviewer, { reviewer: draft.reviewer })}</li>
          <li>
            {intl.formatMessage(m.sdApprovals, { given: draft.approvers.length, required: draft.approvalsRequired })}
            {draft.approvers.length === 0 ? null : ` · ${intl.formatMessage(m.sdApprovers, { approvers: draft.approvers.join(', ') })}`}
          </li>
        </ul>
        <Actions id={id} draft={draft} />
      </section>

      <Discussion id={id} draft={draft} workflows={workflows} time={time} />
    </>
  )
}

/**
 * The draft's one discussion (#496): the history the lifecycle wrote and the comments among it,
 * oldest first, then a box for a new comment. A comment naming an artefact links to it; the
 * server writes the entry, so the client sends only the text and the artefact.
 */
function Discussion({ id, draft, workflows, time }: { id: string; draft: StudioPackagePage; workflows: Set<string>; time: (iso: string) => string }) {
  const intl = useIntl()
  return (
    <section aria-labelledby="studio-discussion" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <h2 id="studio-discussion" className={subheading}>
        {intl.formatMessage(m.sdDiscussion)}
      </h2>
      <ol className={list}>
        {draft.history.map((entry, index) =>
          entry.step === 'commented' ? (
            <li key={`${entry.at}:${index}`}>
              {intl.formatMessage(m.sdCommented, { by: entry.by, at: time(entry.at) })}
              <p className={css({ whiteSpace: 'pre-wrap' })}>{entry.text}</p>
              {entry.about === null || entry.about === undefined ? null : (
                <p>
                  {intl.formatMessage(m.sdCommentAbout)} <ArtefactLink id={id} draft={draft} workflows={workflows} path={entry.about} />
                </p>
              )}
            </li>
          ) : (
            <li key={`${entry.at}:${index}`}>
              {intl.formatMessage(m.sdHistoryEntry, { step: entry.step, by: entry.by, at: time(entry.at) })}
              {entry.person === null || entry.person === undefined ? null : <p>{intl.formatMessage(m.sdHistoryPerson, { person: entry.person })}</p>}
              {entry.reason === null || entry.reason === undefined ? null : <p>{entry.reason}</p>}
              {entry.problems.length === 0 ? null : (
                <ul className={list}>
                  {entry.problems.map((problem) => (
                    <li key={problem}>{problem}</li>
                  ))}
                </ul>
              )}
            </li>
          ),
        )}
      </ol>
      <CommentBox id={id} draft={draft} />
    </section>
  )
}

/** An artefact a comment names, opening in its editor: the form designer, the terminology
 * browser, the Workflow Studio, or else the file editor. Its link reads as its path; one the
 * draft no longer has is its path alone. */
function ArtefactLink({ id, draft, workflows, path }: { id: string; draft: StudioPackagePage; workflows: Set<string>; path: string }) {
  const file = draft.files.find((f) => f.path === path)
  if (file === undefined) return <code>{path}</code>
  const type = file.resourceType === null || file.resourceType === undefined ? null : <span className={muted}> · {file.resourceType}</span>
  const to =
    file.resourceType === 'Questionnaire'
      ? ('/studio/$id/forms/$' as const)
      : isTerminology(file.resourceType)
        ? ('/studio/$id/terminology/$' as const)
        : workflows.has(path)
          ? ('/studio/$id/workflows/$' as const)
          : isProgram(draft, file.resourceType)
            ? ('/studio/$id/programs/$' as const)
            : file.resourceType === 'Measure'
              ? ('/studio/$id/indicators/$' as const)
              : ('/studio/$id/files/$' as const)
  return (
    <>
      <Link to={to} params={{ id, _splat: path }}>
        {path}
      </Link>
      {type}
    </>
  )
}

/** The box for a new comment, naming one of the draft's artefacts if it is about one. Closed,
 * with the server's reason, once the draft is published or withdrawn. */
function CommentBox({ id, draft }: { id: string; draft: StudioPackagePage }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const textId = useId()
  const aboutId = useId()
  const [text, setText] = useState('')
  const [about, setAbout] = useState('')
  const post = useMutation({
    mutationFn: () => addComment(platform.transport, id, text.trim(), about),
    onSuccess: (next) => {
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      setText('')
      setAbout('')
    },
  })
  if (!draft.commentable) {
    return draft.commentRefusal === null || draft.commentRefusal === undefined ? null : (
      <p className={muted}>{intl.formatMessage(m.sdCommentClosed, { reason: refusalText(intl, draft.commentRefusal) })}</p>
    )
  }
  const artefacts = draft.files.filter((f) => f.resourceType !== null && f.resourceType !== undefined)
  const why = post.error instanceof StudioError ? (post.error.refusal === null || post.error.refusal === undefined ? post.error.detail : refusalText(intl, post.error.refusal)) : ''
  return (
    <form
      className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}
      onSubmit={(e) => {
        e.preventDefault()
        if (text.trim() !== '') post.mutate()
      }}
    >
      <div className={field}>
        <label htmlFor={textId}>{intl.formatMessage(m.sdCommentLabel)}</label>
        <textarea id={textId} className={control} value={text} onChange={(e) => setText(e.target.value)} />
      </div>
      <div className={field}>
        <label htmlFor={aboutId}>{intl.formatMessage(m.sdCommentAboutLabel)}</label>
        <select id={aboutId} className={control} value={about} onChange={(e) => setAbout(e.target.value)}>
          <option value="">{intl.formatMessage(m.sdCommentAboutNone)}</option>
          {artefacts.map((file) => (
            <option key={file.path} value={file.path}>
              {`${file.resourceType ?? ''} ${file.path}`}
            </option>
          ))}
        </select>
      </div>
      <button type="submit" className={button} disabled={post.isPending || text.trim() === ''}>
        {intl.formatMessage(m.sdCommentPost)}
      </button>
      {post.isSuccess ? <p role="status">{intl.formatMessage(m.sdCommentPosted)}</p> : null}
      {post.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.sdCommentFailed, { reason: why })}
        </p>
      ) : null}
    </form>
  )
}

/** The package's files, each opening in the editor; in a draft, a new file and the version. */
/** Opens a Questionnaire in the form designer (#497). */
function DesignerLink({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  return (
    <Link to="/studio/$id/forms/$" params={{ id, _splat: path }} aria-label={intl.formatMessage(m.fdOpen, { path })}>
      {intl.formatMessage(m.fdOpenShort)}
    </Link>
  )
}

const isTerminology = (type: string | null | undefined) => type === 'CodeSystem' || type === 'ValueSet'

/** Opens a CodeSystem or ValueSet in the terminology browser (#523). */
function TerminologyLink({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  return (
    <Link to="/studio/$id/terminology/$" params={{ id, _splat: path }} aria-label={intl.formatMessage(tm.tbOpen, { path })}>
      {intl.formatMessage(tm.tbOpenShort)}
    </Link>
  )
}

/** A program package's PlanDefinitions open in the Program Designer (#500), which says so if
 * one is not a clinical protocol. */
const isProgram = (draft: StudioPackagePage, type: string | null | undefined) => draft.package.kind === 'program' && type === 'PlanDefinition'

/** Opens a program's PlanDefinition in the Program Designer (#500). */
function ProgramLink({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  return (
    <Link to="/studio/$id/programs/$" params={{ id, _splat: path }} aria-label={intl.formatMessage(pm.pdOpen, { path })}>
      {intl.formatMessage(pm.pdOpenShort)}
    </Link>
  )
}

/** Opens a Measure in the indicator editor (#537). */
function IndicatorLink({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  return (
    <Link to="/studio/$id/indicators/$" params={{ id, _splat: path }} aria-label={intl.formatMessage(im.inOpen, { path })}>
      {intl.formatMessage(im.inOpenShort)}
    </Link>
  )
}

/** Opens a Workflow Definition in the Workflow Studio (#498). */
function WorkflowLink({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  return (
    <Link to="/studio/$id/workflows/$" params={{ id, _splat: path }} aria-label={intl.formatMessage(wm.wsOpen, { path })}>
      {intl.formatMessage(wm.wsOpenShort)}
    </Link>
  )
}

/** The Draft Holder's one change note for the whole draft (#495): shown to everyone, and
 * edited by whoever may edit the draft, under the same revision check as a save. */
function ChangeNote({ id, draft }: { id: string; draft: StudioPackagePage }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const noteId = useId()
  const hintId = useId()
  const saved = draft.note ?? ''
  const why = (error: unknown) => (error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail : refusalText(intl, error.refusal)) : '')
  const [note, setNoteText] = useState(saved)
  const save = useMutation({
    mutationFn: () => setNote(platform.transport, id, note.trim(), draft.revision),
    onSuccess: (next) => {
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
    },
  })
  if (!draft.editable) {
    return saved === '' ? null : (
      <div>
        <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.sdNoteHeading)}</h3>
        <p className={css({ whiteSpace: 'pre-wrap' })}>{saved}</p>
      </div>
    )
  }
  return (
    <form
      className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}
      onSubmit={(e) => {
        e.preventDefault()
        save.mutate()
      }}
    >
      <div className={field}>
        <label htmlFor={noteId}>{intl.formatMessage(m.sdNoteLabel)}</label>
        <textarea id={noteId} className={control} value={note} aria-describedby={hintId} onChange={(e) => setNoteText(e.target.value)} />
        <span id={hintId} className={muted}>
          {intl.formatMessage(m.sdNoteHint)}
        </span>
      </div>
      <button type="submit" className={button} disabled={save.isPending || note.trim() === saved}>
        {intl.formatMessage(m.sdNoteSave)}
      </button>
      {save.isSuccess && note.trim() === saved ? <p role="status">{intl.formatMessage(m.sdNoteSaved)}</p> : null}
      {save.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.sdEditorFailed, { reason: why(save.error) })}
        </p>
      ) : null}
    </form>
  )
}

function Files({ id, draft, workflows }: { id: string; draft: StudioPackagePage; workflows: Set<string> }) {
  const intl = useIntl()
  const platform = usePlatform()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const pathId = useId()
  const pathHintId = useId()
  const versionId = useId()
  const [path, setPath] = useState('')
  const [version, setVersionText] = useState(draft.package.version)
  const renumber = useMutation({
    mutationFn: () => setVersion(platform.transport, id, version.trim(), draft.revision),
    onSuccess: async (next) => {
      queryClient.setQueryData(packageQuery(platform.transport, next.package.id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
      if (next.package.id !== id) await navigate({ to: '/studio/$id', params: { id: next.package.id } })
    },
  })
  const copy = useMutation({
    mutationFn: () => startDraft(platform.transport, { copyOf: id }),
    onSuccess: async (next) => {
      queryClient.setQueryData(packageQuery(platform.transport, next.package.id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
      await navigate({ to: '/studio/$id', params: { id: next.package.id } })
    },
  })
  const why = (error: unknown) => (error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail : refusalText(intl, error.refusal)) : '')
  const published = draft.package.stage === 'published'
  return (
    <section aria-labelledby="studio-files" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <h2 id="studio-files" className={subheading}>
        {intl.formatMessage(m.sdFiles)}
      </h2>
      <p className={muted}>{intl.formatMessage(m.sdFilesHint)}</p>
      {draft.editable || published || draft.editRefusal === null || draft.editRefusal === undefined ? null : (
        <p className={muted}>{intl.formatMessage(m.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}</p>
      )}
      <ul className={list}>
        {draft.files.map((file) => (
          <li key={file.path}>
            <Link to="/studio/$id/files/$" params={{ id, _splat: file.path }} aria-label={intl.formatMessage(m.sdOpenFile, { path: file.path })}>
              {file.path}
            </Link>
            {file.resourceType === null || file.resourceType === undefined ? null : (
              <span className={muted}>
                {' '}
                · {file.resourceType}
                {file.version === null || file.version === undefined ? '' : ` ${file.version}`}
              </span>
            )}
            {file.resourceType === 'Questionnaire' ? (
              <>
                {' · '}
                <DesignerLink id={id} path={file.path} />
              </>
            ) : null}
            {isTerminology(file.resourceType) ? (
              <>
                {' · '}
                <TerminologyLink id={id} path={file.path} />
              </>
            ) : null}
            {workflows.has(file.path) ? (
              <>
                {' · '}
                <WorkflowLink id={id} path={file.path} />
              </>
            ) : null}
            {isProgram(draft, file.resourceType) ? (
              <>
                {' · '}
                <ProgramLink id={id} path={file.path} />
              </>
            ) : null}
            {file.resourceType === 'Measure' ? (
              <>
                {' · '}
                <IndicatorLink id={id} path={file.path} />
              </>
            ) : null}
          </li>
        ))}
      </ul>
      {draft.editable ? (
        <>
          <form
            className={css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'flex-end' })}
            onSubmit={(e) => {
              e.preventDefault()
              if (path.trim() !== '') void navigate({ to: '/studio/$id/files/$', params: { id, _splat: path.trim() } })
            }}
          >
            <div className={field}>
              <label htmlFor={pathId}>{intl.formatMessage(m.sdNewFilePath)}</label>
              <input id={pathId} className={control} value={path} aria-describedby={pathHintId} onChange={(e) => setPath(e.target.value)} />
              <span id={pathHintId} className={muted}>
                {intl.formatMessage(m.sdNewFilePathHint)}
              </span>
            </div>
            <button type="submit" className={button} disabled={path.trim() === ''}>
              {intl.formatMessage(m.sdAddFile)}
            </button>
          </form>
          <form
            className={css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'flex-end' })}
            onSubmit={(e) => {
              e.preventDefault()
              renumber.mutate()
            }}
          >
            <div className={field}>
              <label htmlFor={versionId}>{intl.formatMessage(m.sdVersion)}</label>
              <input id={versionId} className={control} value={version} onChange={(e) => setVersionText(e.target.value)} />
            </div>
            <button type="submit" className={button} disabled={renumber.isPending || version.trim() === '' || version.trim() === draft.package.version}>
              {intl.formatMessage(m.sdChangeVersion)}
            </button>
          </form>
          {renumber.isSuccess ? <p role="status">{intl.formatMessage(m.sdVersionChanged)}</p> : null}
          {renumber.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {intl.formatMessage(m.sdEditorFailed, { reason: why(renumber.error) })}
            </p>
          ) : null}
        </>
      ) : null}
      {published ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
          <button type="button" className={button} disabled={copy.isPending} onClick={() => copy.mutate()}>
            {intl.formatMessage(m.sdStartNext)}
          </button>
          <span className={muted}>{intl.formatMessage(m.sdStartNextHint)}</span>
          {copy.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {intl.formatMessage(m.sdStartFailed, { reason: why(copy.error) })}
            </p>
          ) : null}
        </div>
      ) : null}
    </section>
  )
}

/** The server's actions, each enabled or disabled with its reason. */
function Actions({ id, draft }: { id: string; draft: StudioPackagePage }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const reasonId = useId()
  const scopeId = useId()
  const scopeHintId = useId()
  const toId = useId()
  const toHintId = useId()
  const [reason, setReason] = useState('')
  const [scope, setScope] = useState('national')
  const [to, setTo] = useState('')
  const step = useMutation({
    mutationFn: (action: StudioActionName) => takeAction(platform.transport, id, action, actionInput(action, { reason, scope, to })),
    onSuccess: async (taken) => {
      queryClient.setQueryData(packageQuery(platform.transport, taken.package.id).queryKey, taken)
      setReason('')
      setTo('')
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
      // A withdrawn draft is kept under another id, which the page moves to.
      if (taken.package.id !== id) await navigate({ to: '/studio/$id', params: { id: taken.package.id } })
    },
  })
  const offered = (action: StudioActionName) => draft.package.actions.some((a) => a.action === action && a.allowed)
  const asked = (wanted: 'reason' | 'to') => draft.package.actions.some((a) => a.allowed && needs(a.action).includes(wanted))
  const failure = step.error instanceof StudioError ? (step.error.refusal === null || step.error.refusal === undefined ? step.error.detail : refusalText(intl, step.error.refusal)) : ''
  return (
    <>
      {asked('reason') ? (
        <div className={field}>
          <label htmlFor={reasonId}>{intl.formatMessage(m.sdReason)}</label>
          <textarea id={reasonId} className={control} value={reason} onChange={(e) => setReason(e.target.value)} />
        </div>
      ) : null}
      {asked('to') ? (
        <div className={field}>
          <label htmlFor={toId}>{intl.formatMessage(m.sdTo)}</label>
          <input id={toId} className={control} value={to} aria-describedby={toHintId} autoComplete="off" onChange={(e) => setTo(e.target.value)} />
          <span id={toHintId} className={muted}>
            {intl.formatMessage(m.sdToHint)}
          </span>
        </div>
      ) : null}
      {offered('activate') ? (
        <div className={field}>
          <label htmlFor={scopeId}>{intl.formatMessage(m.sdScope)}</label>
          <input id={scopeId} className={control} value={scope} aria-describedby={scopeHintId} onChange={(e) => setScope(e.target.value)} />
          <span id={scopeHintId} className={muted}>
            {intl.formatMessage(m.sdScopeHint)}
          </span>
        </div>
      ) : null}
      <ul className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {draft.package.actions.map((action) => {
          const why = action.refusal === null || action.refusal === undefined ? undefined : refusalText(intl, action.refusal)
          const whyId = `studio-why-${action.action}`
          return (
            <li key={action.action} className={css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center' })}>
              <button
                type="button"
                className={button}
                disabled={!action.allowed || step.isPending}
                aria-describedby={why === undefined ? undefined : whyId}
                onClick={() => step.mutate(action.action)}
              >
                {actionLabel(intl, action.action)}
              </button>
              {why === undefined ? null : (
                <span id={whyId} className={muted}>
                  {why}
                </span>
              )}
            </li>
          )
        })}
      </ul>
      {step.isSuccess ? <p role="status">{intl.formatMessage(m.sdDone)}</p> : null}
      {step.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.sdActionFailed, { reason: failure })}
        </p>
      ) : null}
    </>
  )
}
