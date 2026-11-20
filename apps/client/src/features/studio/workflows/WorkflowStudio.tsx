import { css, cx } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useId, useRef, useState } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { studioMessages as sm, workflowMessages as m } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button, control } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { refusalText } from '../wording'
import {
  KINDS,
  PRIORITIES,
  activityOf,
  addState,
  addTransition,
  competitors,
  descendants,
  edgeId,
  entryActions,
  flagged,
  moveSource,
  moveState,
  moveTransition,
  parentOf,
  problemAt,
  problems,
  publishedIds,
  readDefinition,
  removeState,
  removeTransition,
  renameState,
  rows,
  stateOf,
  transitionAt,
  triggerOf,
  updateState,
  updateTransition,
  withActivity,
  withEntryActions,
  withKind,
  withTrigger,
  writeDefinition,
  type Action,
  type Activity,
  type Kind,
  type Library,
  type Priority,
  type Problem,
  type Selection,
  type State,
  type Transition,
  type Value,
  type WorkflowDefinition,
} from './model'
import { WorkflowCanvas } from './WorkflowCanvas'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const panel = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface', p: '3', display: 'flex', flexDirection: 'column', gap: '3', minWidth: '0' })
const columns = css({ display: 'grid', gap: '4', alignItems: 'start', gridTemplateColumns: { base: '1fr', xl: '17rem minmax(0, 1fr) 24rem' } })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1' })
const input = cx(control, css({ width: '100%' }))
const fieldGroup = css({ display: 'flex', flexDirection: 'column', gap: '2', border: 'none', p: '0', m: '0' })
const legend = css({ fontWeight: 'semibold', mb: '1' })
const row = css({ display: 'flex', alignItems: 'center', gap: '1', flexWrap: 'wrap' })
const linkButton = css({ textDecoration: 'underline', cursor: 'pointer', textAlign: 'left', overflowWrap: 'anywhere' })
const small = css({
  minHeight: 'control',
  minWidth: 'control',
  px: '2',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})
const outlineButton = css({
  flex: '1',
  textAlign: 'left',
  p: '1',
  borderRadius: 'sm',
  cursor: 'pointer',
  minWidth: '0',
  overflowWrap: 'anywhere',
  '&[aria-current=true]': { bg: 'accent.subtle', fontWeight: 'semibold' },
  '&[data-flagged=true]': { outline: '2px solid', outlineColor: 'danger.text' },
})

/**
 * The Workflow Studio (studio.md § The Workflow Studio; #498): a draft's Workflow Definition in
 * three columns, the states as a tree with their exits on the left, the statechart on the canvas
 * in the middle and the selected state or transition in the inspector on the right, with the
 * publication checks above, rerun on every edit. It saves the Library through the same path as
 * the file editor (#494), with the draft's revision as `If-Match`, so a stale tab is refused with
 * 412 and keeps nothing. A state the previous published version has keeps its id.
 */
export function WorkflowStudio({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  const file = useQuery({ ...fileQuery(platform.transport, id, path), retry: false })
  const previousId = draft.data?.package.previous === null || draft.data?.package.previous === undefined ? null : `${draft.data.package.name}@${draft.data.package.previous}`
  const previous = useQuery({ ...fileQuery(platform.transport, previousId ?? '', path), enabled: previousId !== null, retry: false })
  const previousSettled = previousId === null || previous.data !== undefined || (previous.error instanceof StudioError && previous.error.status === 404)
  const opened = useRef(false)
  const [reloads, setReloads] = useState(0)
  const loading = !opened.current && (draft.data === undefined || file.data === undefined || !previousSettled)
  const failed = draft.isError || file.isError || (previous.isError && !previousSettled)
  if (!loading && draft.data !== undefined && file.data !== undefined) opened.current = true
  const published = previous.data === undefined ? null : readDefinition(previous.data.content)
  return (
    <section className={page}>
      <div className={css({ display: 'flex', gap: '4', flexWrap: 'wrap' })}>
        <Link to="/studio/$id" params={{ id }}>
          {intl.formatMessage(sm.sdEditorBack)}
        </Link>
        <Link to="/studio/$id/files/$" params={{ id, _splat: path }}>
          {intl.formatMessage(sm.fdAsJson)}
        </Link>
      </div>
      {loading || draft.data === undefined || file.data === undefined ? (
        failed ? (
          <p role="alert" className={alert}>
            {intl.formatMessage(sm.sdFailed)}
          </p>
        ) : (
          <p role="status">{intl.formatMessage(m.wsLoading)}</p>
        )
      ) : (
        <Loaded
          key={reloads}
          id={id}
          path={path}
          draft={draft.data}
          file={file.data}
          published={published !== null && 'definition' in published ? published.definition : null}
          onReload={async () => {
            await Promise.all([draft.refetch(), file.refetch()])
            setReloads((n) => n + 1)
          }}
        />
      )}
    </section>
  )
}

interface LoadedProps {
  id: string
  path: string
  draft: StudioPackagePage
  file: StudioFile
  published: WorkflowDefinition | null
  onReload: () => Promise<void>
}

function Loaded(props: LoadedProps) {
  const intl = useIntl()
  const read = readDefinition(props.file.content)
  if ('refusal' in read)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.wsRefused, { refusal: read.refusal })}
      </p>
    )
  return <Studio {...props} library={read.library} initial={read.definition} />
}

const sameSelection = (a: Selection | null, b: Selection) =>
  a !== null && (a.kind === 'state' ? b.kind === 'state' && a.id === b.id : b.kind === 'transition' && a.source === b.source && a.index === b.index)

function Studio({ id, path, draft, file, published, onReload, library, initial }: LoadedProps & { library: Library; initial: WorkflowDefinition }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const [definition, setDefinition] = useState(initial)
  const [saved, setSaved] = useState(() => JSON.stringify(initial))
  const [revision, setRevision] = useState(file.revision)
  const [selected, setSelected] = useState<Selection | null>(() => (initial.statechart.states[0] === undefined ? null : { kind: 'state', id: initial.statechart.states[0].id }))
  const editable = draft.editable
  const text = JSON.stringify(definition)
  const dirty = text !== saved
  const found = problems(definition)
  const outlined = flagged(found)
  const fixed = publishedIds(published)

  const save = useMutation({
    mutationFn: () => saveFile(platform.transport, id, path, JSON.stringify(writeDefinition(library, definition), null, 2), revision),
    onSuccess: async (next) => {
      setRevision(next.revision)
      setSaved(text)
      queryClient.setQueryData(fileQuery(platform.transport, id, path).queryKey, { path, content: JSON.stringify(writeDefinition(library, definition), null, 2), revision: next.revision })
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
  const stale = save.error instanceof StudioError && save.error.status === 412
  const reason =
    save.error instanceof StudioError ? (save.error.refusal === null || save.error.refusal === undefined ? save.error.detail : refusalText(intl, save.error.refusal)) : save.error === null ? '' : String(save.error)

  const change = (next: WorkflowDefinition) => {
    setDefinition(next)
    save.reset()
  }
  const connect = (source: string, target: string) => {
    const added = addTransition(definition, source, target)
    change(added.definition)
    setSelected({ kind: 'transition', source, index: added.index })
  }

  return (
    <>
      <header className={css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center', justifyContent: 'space-between' })}>
        <div>
          <h1 className={heading}>{intl.formatMessage(m.wsHeading, { title: typeof library['title'] === 'string' ? library['title'] : path })}</h1>
          <p className={muted}>{intl.formatMessage(m.wsFile, { path, url: definition.url, version: definition.version })}</p>
        </div>
        {editable ? (
          <div className={row}>
            {dirty ? <span className={muted}>{intl.formatMessage(sm.fdUnsaved)}</span> : null}
            <button type="button" className={button} disabled={!dirty || save.isPending || stale} onClick={() => save.mutate()}>
              {intl.formatMessage(sm.fdSave)}
            </button>
          </div>
        ) : null}
      </header>
      {editable ? null : (
        <p className={muted}>
          {draft.editRefusal === null || draft.editRefusal === undefined ? intl.formatMessage(sm.sdEditorReadOnly) : intl.formatMessage(sm.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}
        </p>
      )}
      {save.isSuccess && !dirty ? <p role="status">{intl.formatMessage(sm.fdSaved)}</p> : null}
      {stale ? (
        <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
          <p className={alert}>{intl.formatMessage(sm.sdEditorStale)}</p>
          <button type="button" className={button} onClick={() => void onReload()}>
            {intl.formatMessage(sm.sdEditorReload)}
          </button>
        </div>
      ) : save.error === null ? null : (
        <p role="alert" className={alert}>
          {intl.formatMessage(sm.sdEditorFailed, { reason })}
        </p>
      )}

      <Problems found={found} onSelect={setSelected} />

      <div className={columns}>
        <Outline definition={definition} selected={selected} outlined={outlined} editable={editable} onSelect={setSelected} onChange={change} />
        <section aria-labelledby="ws-canvas" className={panel}>
          <h2 id="ws-canvas" className={subheading}>
            {intl.formatMessage(m.wsCanvasHeading)}
          </h2>
          {editable ? <p className={muted}>{intl.formatMessage(m.wsCanvasHint)}</p> : null}
          <WorkflowCanvas definition={definition} selected={selected} flagged={outlined} onSelect={setSelected} onConnect={editable ? connect : undefined} />
        </section>
        <section aria-labelledby="ws-inspect" className={panel}>
          <h2 id="ws-inspect" className={subheading}>
            {intl.formatMessage(m.wsInspect)}
          </h2>
          {selected === null ? (
            <p className={muted}>{intl.formatMessage(m.wsChoose)}</p>
          ) : selected.kind === 'state' ? (
            <StateInspector
              key={`state:${selected.id}`}
              definition={definition}
              id={selected.id}
              fixed={fixed.has(selected.id)}
              editable={editable}
              onChange={change}
              onSelect={setSelected}
            />
          ) : (
            <TransitionInspector key={`transition:${selected.source}:${selected.index}`} definition={definition} source={selected.source} index={selected.index} editable={editable} onChange={change} onSelect={setSelected} />
          )}
        </section>
      </div>
    </>
  )
}

function problemText(intl: IntlShape, problem: Problem): string {
  switch (problem.code) {
    case 'no-id':
      return intl.formatMessage(m.wsPNoId)
    case 'duplicate-id':
      return intl.formatMessage(m.wsPDuplicate, { id: problem.state })
    case 'bad-children':
      return intl.formatMessage(m.wsPChildren, { id: problem.state })
    case 'bad-initial':
      return intl.formatMessage(m.wsPInitial, { id: problem.state })
    case 'unknown-target':
      return intl.formatMessage(m.wsPTarget, { source: problem.source, target: problem.target })
    case 'unknown-initial':
      return intl.formatMessage(m.wsPStart, { target: problem.target })
    case 'unreachable':
      return intl.formatMessage(m.wsPUnreachable, { id: problem.state })
    case 'dead-end':
      return intl.formatMessage(m.wsPDeadEnd, { id: problem.state })
    case 'bad-task':
      return intl.formatMessage(m.wsPTask, { id: problem.state })
    case 'bad-gate':
      return intl.formatMessage(m.wsPGate, { id: problem.state })
    case 'bad-assignment':
      return intl.formatMessage(m.wsPAssignment, { field: problem.field })
    case 'bad-table':
      return intl.formatMessage(m.wsPTable, { id: problem.table })
  }
}

/** The publication checks the browser can run, each selecting the element it is about. */
function Problems({ found, onSelect }: { found: Problem[]; onSelect: (selection: Selection) => void }) {
  const intl = useIntl()
  return (
    <section aria-labelledby="ws-problems" className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <h2 id="ws-problems" className={subheading}>
        {intl.formatMessage(m.wsProblems, { count: found.length })}
      </h2>
      <p className={muted}>{intl.formatMessage(m.wsProblemsHint)}</p>
      {found.length === 0 ? null : (
        <ul className={css({ listStyle: 'disc', pl: '5', display: 'flex', flexDirection: 'column', gap: '1' })}>
          {found.map((problem, n) => {
            const at = problemAt(problem)
            const text = problemText(intl, problem)
            return (
              <li key={`${problem.code}:${n}`}>
                {at === null ? (
                  text
                ) : (
                  <button type="button" className={linkButton} onClick={() => onSelect(at)}>
                    {text}
                  </button>
                )}
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}

/** What a transition is taken on, in words. */
function onText(intl: IntlShape, state: State, transition: Transition): string {
  const trigger = triggerOf(state, transition)
  if (trigger.kind === 'eventless') return intl.formatMessage(m.wsEventless)
  if (trigger.kind === 'timer') return intl.formatMessage(m.wsAfter, { after: trigger.after })
  return trigger.events.join(', ')
}

const targetText = (transition: Transition) => (transition.targets ?? []).join(', ') || '—'
const guardText = (transition: Transition) => (transition.guard === undefined || transition.guard === null || transition.guard === '' ? '' : `[${transition.guard}]`)

interface OutlineProps {
  definition: WorkflowDefinition
  selected: Selection | null
  outlined: Set<string>
  editable: boolean
  onSelect: (selection: Selection) => void
  onChange: (definition: WorkflowDefinition) => void
}

/** The states as a tree, each with its exits under it. */
function Outline({ definition, selected, outlined, editable, onSelect, onChange }: OutlineProps) {
  const intl = useIntl()
  const add = (parent: string | null) => {
    const added = addState(definition, parent)
    onChange(added.definition)
    onSelect({ kind: 'state', id: added.id })
  }
  return (
    <nav aria-labelledby="ws-states" className={panel}>
      <h2 id="ws-states" className={subheading}>
        {intl.formatMessage(m.wsStates)}
      </h2>
      <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        {rows(definition).map(({ state, depth }) => (
          <li key={state.id} style={{ paddingLeft: `${depth * 1.25}rem` }} className={css({ display: 'flex', flexDirection: 'column', gap: '0.5' })}>
            <button
              type="button"
              aria-current={sameSelection(selected, { kind: 'state', id: state.id }) ? 'true' : undefined}
              data-flagged={outlined.has(state.id)}
              className={outlineButton}
              onClick={() => onSelect({ kind: 'state', id: state.id })}
            >
              {state.id} <span className={muted}>{intl.formatMessage(m.wsKindName, { kind: state.kind })}</span>
            </button>
            {(state.transitions ?? []).length === 0 ? null : (
              <ul className={css({ pl: '3', display: 'flex', flexDirection: 'column' })}>
                {(state.transitions ?? []).map((transition, index) => (
                  <li key={index}>
                    <button
                      type="button"
                      aria-current={sameSelection(selected, { kind: 'transition', source: state.id, index }) ? 'true' : undefined}
                      data-flagged={outlined.has(edgeId(state.id, index))}
                      className={cx(outlineButton, muted)}
                      onClick={() => onSelect({ kind: 'transition', source: state.id, index })}
                    >
                      {intl.formatMessage(m.wsExit, { target: targetText(transition), on: onText(intl, state, transition) })} {guardText(transition)}
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {editable && (state.kind === 'compound' || state.kind === 'parallel') ? (
              <button type="button" className={cx(small, css({ alignSelf: 'flex-start', ml: '3' }))} onClick={() => add(state.id)}>
                {intl.formatMessage(m.wsAddInside, { id: state.id })}
              </button>
            ) : null}
          </li>
        ))}
      </ul>
      {editable ? (
        <button type="button" className={small} onClick={() => add(null)}>
          {intl.formatMessage(m.wsAddState)}
        </button>
      ) : null}
    </nav>
  )
}

interface StateInspectorProps {
  definition: WorkflowDefinition
  id: string
  fixed: boolean
  editable: boolean
  onChange: (definition: WorkflowDefinition) => void
  onSelect: (selection: Selection | null) => void
}

function StateInspector({ definition, id, fixed, editable, onChange, onSelect }: StateInspectorProps) {
  const intl = useIntl()
  const ids = { id: useId(), kind: useId(), parent: useId(), initial: useId(), activity: useId(), target: useId(), idHint: useId() }
  const [name, setName] = useState(id)
  const [target, setTarget] = useState('')
  const state = stateOf(definition, id)
  if (state === undefined) return <p className={muted}>{intl.formatMessage(m.wsChoose)}</p>
  const others = rows(definition).filter((r) => r.state.id !== id)
  const used = others.some((r) => r.state.id === name.trim())
  const edit = (f: (s: State) => State) => onChange(updateState(definition, id, f))
  const parent = parentOf(definition, id)
  const inside = descendants(definition, id)
  const containers = others.filter((r) => (r.state.kind === 'compound' || r.state.kind === 'parallel') && !inside.has(r.state.id))
  const activity = activityOf(state)
  const task = state.task ?? null
  const container = (state.children ?? []).length > 0
  const chosenTarget = target === '' ? (rows(definition)[0]?.state.id ?? '') : target
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.wsStateHeading, { id })}</h3>
      <label className={label} htmlFor={ids.id}>
        {intl.formatMessage(m.wsId)}
        <input
          id={ids.id}
          className={input}
          value={name}
          disabled={!editable || fixed}
          aria-describedby={ids.idHint}
          aria-invalid={used || name.trim() === '' ? 'true' : undefined}
          onChange={(e) => setName(e.target.value)}
          // Renamed when the field is left, so the inspector keeps focus while typing.
          onBlur={() => {
            const next = name.trim()
            if (next !== '' && next !== id && !used) {
              onChange(renameState(definition, id, next))
              onSelect({ kind: 'state', id: next })
            }
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') e.currentTarget.blur()
          }}
        />
        <span id={ids.idHint} className={used || name.trim() === '' ? alert : muted}>
          {fixed ? intl.formatMessage(m.wsIdFixed) : used ? intl.formatMessage(m.wsIdUsed) : name.trim() === '' ? intl.formatMessage(m.wsIdEmpty) : ''}
        </span>
      </label>
      <label className={label} htmlFor={ids.kind}>
        {intl.formatMessage(m.wsKind)}
        <select id={ids.kind} className={input} value={state.kind} disabled={!editable} onChange={(e) => edit((s) => withKind(s, e.target.value as Kind))}>
          {KINDS.map((kind) => (
            <option key={kind} value={kind}>
              {intl.formatMessage(m.wsKindName, { kind })}
            </option>
          ))}
        </select>
      </label>
      <label className={label} htmlFor={ids.parent}>
        {intl.formatMessage(m.wsParent)}
        <select id={ids.parent} className={input} value={parent ?? ''} disabled={!editable} onChange={(e) => onChange(moveState(definition, id, e.target.value === '' ? null : e.target.value))}>
          <option value="">{intl.formatMessage(m.wsTopLevel)}</option>
          {containers.map((r) => (
            <option key={r.state.id} value={r.state.id}>
              {r.state.id}
            </option>
          ))}
        </select>
      </label>
      {state.kind === 'compound' && container ? (
        <label className={label} htmlFor={ids.initial}>
          {intl.formatMessage(m.wsInitial)}
          <select id={ids.initial} className={input} value={state.initial?.[0] ?? state.children?.[0]?.id ?? ''} disabled={!editable} onChange={(e) => edit((s) => ({ ...s, initial: [e.target.value] }))}>
            {(state.children ?? []).map((child) => (
              <option key={child.id} value={child.id}>
                {child.id}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {state.kind === 'atomic' ? (
        <>
          <label className={label} htmlFor={ids.activity}>
            {intl.formatMessage(m.wsActivity)}
            <select id={ids.activity} className={input} value={activity} disabled={!editable} onChange={(e) => edit((s) => withActivity(s, e.target.value as Activity))}>
              {(['wait', 'task', 'gate', 'invoke'] as const).map((a) => (
                <option key={a} value={a}>
                  {intl.formatMessage(m.wsActivityName, { activity: a })}
                </option>
              ))}
            </select>
          </label>
          {activity === 'gate' ? (
            <>
              <TextField label={intl.formatMessage(m.wsGate)} value={state.gate ?? ''} editable={editable} onChange={(gate) => edit((s) => ({ ...s, gate }))} />
              <p className={muted}>{intl.formatMessage(m.wsGateHint)}</p>
            </>
          ) : null}
          {task !== null && (activity === 'task' || activity === 'gate') ? (
            <fieldset className={fieldGroup}>
              <legend className={legend}>{intl.formatMessage(m.wsActivityName, { activity: 'task' })}</legend>
              <TextField label={intl.formatMessage(m.wsRole)} value={task.role} editable={editable} onChange={(role) => edit((s) => ({ ...s, task: { ...task, role } }))} />
              <TextField label={intl.formatMessage(m.wsWorklist)} value={task.worklist} editable={editable} onChange={(worklist) => edit((s) => ({ ...s, task: { ...task, worklist } }))} />
              <TextField
                label={intl.formatMessage(m.wsForm)}
                value={task.form ?? ''}
                editable={editable}
                onChange={(form) =>
                  edit((s) => {
                    const { form: _form, ...rest } = task
                    return { ...s, task: form === '' ? rest : { ...rest, form } }
                  })
                }
              />
              <SelectField
                label={intl.formatMessage(m.wsPriority)}
                value={task.priority ?? 'routine'}
                options={PRIORITIES.map((p) => ({ value: p, text: p }))}
                editable={editable}
                onChange={(priority) => edit((s) => ({ ...s, task: { ...task, priority: priority as Priority } }))}
              />
              <TextField
                label={intl.formatMessage(m.wsClaimTimeout)}
                value={task.claimTimeout ?? 'PT15M'}
                editable={editable}
                onChange={(claimTimeout) => edit((s) => ({ ...s, task: { ...task, claimTimeout } }))}
              />
            </fieldset>
          ) : null}
          {activity === 'invoke' ? (
            <TextField
              label={intl.formatMessage(m.wsInvoke)}
              value={state.invoke?.[0]?.src ?? ''}
              editable={editable}
              onChange={(src) => edit((s) => ({ ...s, invoke: [{ id: s.invoke?.[0]?.id ?? s.id, src }, ...(s.invoke ?? []).slice(1)] }))}
            />
          ) : null}
        </>
      ) : null}
      <ActionsEditor legendText={intl.formatMessage(m.wsEntry)} definition={definition} actions={entryActions(state)} editable={editable} onChange={(actions) => edit((s) => withEntryActions(s, actions))} />
      <fieldset className={fieldGroup}>
        <legend className={legend}>{intl.formatMessage(m.wsExits)}</legend>
        {(state.transitions ?? []).length === 0 ? (
          <p className={muted}>{intl.formatMessage(m.wsNone)}</p>
        ) : (
          <ol className={css({ listStyle: 'decimal', pl: '5' })}>
            {(state.transitions ?? []).map((transition, index) => (
              <li key={index}>
                <button type="button" className={linkButton} onClick={() => onSelect({ kind: 'transition', source: id, index })}>
                  {intl.formatMessage(m.wsExit, { target: targetText(transition), on: onText(intl, state, transition) })} {guardText(transition)}
                </button>
              </li>
            ))}
          </ol>
        )}
        {editable ? (
          <div className={css({ display: 'flex', gap: '2', alignItems: 'flex-end', flexWrap: 'wrap' })}>
            <label className={label} htmlFor={ids.target}>
              {intl.formatMessage(m.wsTarget)}
              <select id={ids.target} className={control} value={chosenTarget} onChange={(e) => setTarget(e.target.value)}>
                {rows(definition).map((r) => (
                  <option key={r.state.id} value={r.state.id}>
                    {r.state.id}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className={small}
              onClick={() => {
                const added = addTransition(definition, id, chosenTarget)
                onChange(added.definition)
                onSelect({ kind: 'transition', source: id, index: added.index })
              }}
            >
              {intl.formatMessage(m.wsAddTransition)}
            </button>
          </div>
        ) : null}
      </fieldset>
      {editable ? (
        <button
          type="button"
          className={cx(small, css({ alignSelf: 'flex-start' }))}
          onClick={() => {
            onChange(removeState(definition, id))
            onSelect(null)
          }}
        >
          {intl.formatMessage(m.wsRemoveState, { id })}
        </button>
      ) : null}
    </div>
  )
}

interface TransitionInspectorProps {
  definition: WorkflowDefinition
  source: string
  index: number
  editable: boolean
  onChange: (definition: WorkflowDefinition) => void
  onSelect: (selection: Selection | null) => void
}

function TransitionInspector({ definition, source, index, editable, onChange, onSelect }: TransitionInspectorProps) {
  const intl = useIntl()
  const ids = { trigger: useId(), guard: useId(), expression: useId() }
  const state = stateOf(definition, source)
  const transition = transitionAt(definition, source, index)
  const [events, setEvents] = useState(() => (transition?.events ?? []).join(', '))
  if (state === undefined || transition === undefined) return <p className={muted}>{intl.formatMessage(m.wsChoose)}</p>
  const trigger = triggerOf(state, transition)
  const edit = (f: (t: Transition) => Transition) => onChange(updateTransition(definition, source, index, f))
  const all = rows(definition).map((r) => ({ value: r.state.id, text: r.state.id }))
  const target = transition.targets?.[0] ?? ''
  const guarded = transition.guard !== undefined && transition.guard !== null
  const rivals = competitors(state, index)
  const order = (state.transitions ?? []).map((t, n) => ({ index: n, transition: t, self: n === index })).filter((entry) => entry.self || rivals.some((r) => r.index === entry.index))
  const readable = [...(definition.dataModel ?? []).map((f) => `%${f.name} (${f.kind})`), ...(definition.dataRequirements ?? []).map((d) => `%${d.name}: ${d.resourceType}${(d.paths ?? []).length > 0 ? ` (${(d.paths ?? []).join(', ')})` : ''}`)]
  const move = (by: -1 | 1) => {
    const moved = moveTransition(definition, source, index, by)
    onChange(moved.definition)
    onSelect({ kind: 'transition', source, index: moved.index })
  }
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.wsTransitionHeading, { source, target: targetText(transition) })}</h3>
      <SelectField
        label={intl.formatMessage(m.wsSource)}
        value={source}
        options={all}
        editable={editable}
        onChange={(to) => {
          const moved = moveSource(definition, source, index, to)
          onChange(moved.definition)
          onSelect({ kind: 'transition', source: to, index: moved.index })
        }}
      />
      <SelectField
        label={intl.formatMessage(m.wsTarget)}
        value={target}
        options={target === '' || all.some((o) => o.value === target) ? all : [{ value: target, text: target }, ...all]}
        editable={editable}
        onChange={(to) => edit((t) => ({ ...t, targets: [to, ...(t.targets ?? []).slice(1)] }))}
      />
      <label className={label} htmlFor={ids.trigger}>
        {intl.formatMessage(m.wsTrigger)}
        <select
          id={ids.trigger}
          className={input}
          value={trigger.kind}
          disabled={!editable}
          onChange={(e) => {
            const kind = e.target.value
            if (kind === 'eventless') onChange(withTrigger(definition, source, index, { kind: 'eventless' }))
            else if (kind === 'timer') onChange(withTrigger(definition, source, index, { kind: 'timer', after: 'PT1H', event: '' }))
            else {
              const named = [`${source}.event`]
              setEvents(named.join(', '))
              onChange(withTrigger(definition, source, index, { kind: 'event', events: named }))
            }
          }}
        >
          {(['event', 'timer', 'eventless'] as const).map((kind) => (
            <option key={kind} value={kind}>
              {intl.formatMessage(m.wsTriggerName, { trigger: kind })}
            </option>
          ))}
        </select>
      </label>
      {trigger.kind === 'event' ? (
        <TextField
          label={intl.formatMessage(m.wsEvents)}
          value={events}
          editable={editable}
          onChange={(value) => {
            setEvents(value)
            const list = value
              .split(',')
              .map((e) => e.trim())
              .filter((e) => e !== '')
            if (list.length > 0) onChange(withTrigger(definition, source, index, { kind: 'event', events: list }))
          }}
        />
      ) : null}
      {trigger.kind === 'timer' ? (
        <>
          <TextField label={intl.formatMessage(m.wsDuration)} value={trigger.after} editable={editable} onChange={(after) => onChange(withTrigger(definition, source, index, { kind: 'timer', after, event: trigger.event }))} />
          <p className={muted}>{intl.formatMessage(m.wsTimerHint)}</p>
        </>
      ) : null}
      <label className={label} htmlFor={ids.guard}>
        {intl.formatMessage(m.wsGuard)}
        <select
          id={ids.guard}
          className={input}
          value={guarded ? 'fhirpath' : 'always'}
          disabled={!editable}
          onChange={(e) =>
            edit((t) => {
              const { guard: _guard, ...rest } = t
              return e.target.value === 'always' ? rest : { ...rest, guard: '' }
            })
          }
        >
          {(['always', 'fhirpath'] as const).map((g) => (
            <option key={g} value={g}>
              {intl.formatMessage(m.wsGuardName, { guard: g })}
            </option>
          ))}
        </select>
      </label>
      {guarded ? (
        <>
          <label className={label} htmlFor={ids.expression}>
            {intl.formatMessage(m.wsExpression)}
            <textarea id={ids.expression} className={input} rows={2} value={transition.guard ?? ''} disabled={!editable} onChange={(e) => edit((t) => ({ ...t, guard: e.target.value }))} />
          </label>
          <div>
            <p className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.wsMayRead)}</p>
            {readable.length === 0 ? (
              <p className={muted}>{intl.formatMessage(m.wsMayReadNone)}</p>
            ) : (
              <ul className={css({ listStyle: 'disc', pl: '5' })}>
                {readable.map((r) => (
                  <li key={r}>
                    <code>{r}</code>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </>
      ) : null}
      <fieldset className={fieldGroup}>
        <legend className={legend}>{intl.formatMessage(m.wsCompetitors)}</legend>
        {rivals.length === 0 ? (
          <p className={muted}>{intl.formatMessage(m.wsNone)}</p>
        ) : (
          <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
            {order.map((entry) => (
              <li key={entry.index}>
                {entry.self ? (
                  <strong>{intl.formatMessage(m.wsThis, { n: entry.index + 1 })}</strong>
                ) : (
                  <button type="button" className={linkButton} onClick={() => onSelect({ kind: 'transition', source, index: entry.index })}>
                    {intl.formatMessage(m.wsCompetitor, { n: entry.index + 1, target: targetText(entry.transition), guard: guardText(entry.transition) })}
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
        {editable ? (
          <div className={row}>
            <button type="button" className={small} disabled={index === 0} onClick={() => move(-1)}>
              {intl.formatMessage(m.wsEarlier)}
            </button>
            <button type="button" className={small} disabled={index === (state.transitions ?? []).length - 1} onClick={() => move(1)}>
              {intl.formatMessage(m.wsLater)}
            </button>
          </div>
        ) : null}
      </fieldset>
      <ActionsEditor legendText={intl.formatMessage(m.wsActions)} definition={definition} actions={transition.actions ?? []} editable={editable} onChange={(actions) => edit((t) => ({ ...t, actions }))} />
      {editable ? (
        <button
          type="button"
          className={cx(small, css({ alignSelf: 'flex-start' }))}
          onClick={() => {
            onChange(removeTransition(definition, source, index))
            onSelect({ kind: 'state', id: source })
          }}
        >
          {intl.formatMessage(m.wsRemoveTransition)}
        </button>
      ) : null}
    </div>
  )
}

/** A value of the kind `field` is declared with, from what was typed. */
function typed(kind: 'bool' | 'int' | 'text' | undefined, text: string): Value {
  if (kind === 'bool') return text === 'true'
  if (kind === 'int') return Number.parseInt(text, 10) || 0
  return text
}

/** Raise and assign actions edited in place; typed actions kept as read (picked from the
 * registered list in the next part). */
function ActionsEditor({ legendText, definition, actions, editable, onChange }: { legendText: string; definition: WorkflowDefinition; actions: Action[]; editable: boolean; onChange: (actions: Action[]) => void }) {
  const intl = useIntl()
  const fields = definition.dataModel ?? []
  const set = (n: number, action: Action) => onChange(actions.map((a, i) => (i === n ? action : a)))
  return (
    <fieldset className={fieldGroup}>
      <legend className={legend}>{legendText}</legend>
      {actions.length === 0 ? <p className={muted}>{intl.formatMessage(m.wsNone)}</p> : null}
      {actions.map((action, n) => (
        <div key={n} className={css({ display: 'flex', flexDirection: 'column', gap: '1', borderLeft: '2px solid', borderColor: 'border', pl: '2' })}>
          <span className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.wsActionKind, { kind: action.kind })}</span>
          {action.kind === 'raise' ? <TextField label={intl.formatMessage(m.wsActionEvent, { n: n + 1 })} value={action.event} editable={editable} onChange={(event) => set(n, { ...action, event })} /> : null}
          {action.kind === 'assign' ? (
            <>
              <SelectField
                label={intl.formatMessage(m.wsActionField, { n: n + 1 })}
                value={action.field}
                options={[...(fields.some((f) => f.name === action.field) ? [] : [{ value: action.field, text: action.field }]), ...fields.map((f) => ({ value: f.name, text: `${f.name} (${f.kind})` }))]}
                editable={editable}
                onChange={(field) => set(n, { ...action, field, value: typed(fields.find((f) => f.name === field)?.kind, String(action.value)) })}
              />
              <TextField
                label={intl.formatMessage(m.wsActionValue, { n: n + 1 })}
                value={String(action.value)}
                editable={editable}
                onChange={(text) => set(n, { ...action, value: typed(fields.find((f) => f.name === action.field)?.kind, text) })}
              />
            </>
          ) : null}
          {action.kind === 'execute' ? (
            <>
              <span>
                {intl.formatMessage(m.wsActionTyped, {
                  n: n + 1,
                  action: action.action,
                  params: Object.entries(action.params ?? {})
                    .map(([k, v]) => `${k}=${String(v)}`)
                    .join(', '),
                })}
              </span>
              <span className={muted}>{intl.formatMessage(m.wsTypedLater)}</span>
            </>
          ) : null}
          {editable ? (
            <button type="button" className={cx(small, css({ alignSelf: 'flex-start' }))} onClick={() => onChange(actions.filter((_, i) => i !== n))}>
              {intl.formatMessage(m.wsRemoveAction, { n: n + 1 })}
            </button>
          ) : null}
        </div>
      ))}
      {editable ? (
        <div className={row}>
          <button type="button" className={small} onClick={() => onChange([...actions, { kind: 'raise', event: '' }])}>
            {intl.formatMessage(m.wsAddRaise)}
          </button>
          <button type="button" className={small} onClick={() => onChange([...actions, { kind: 'assign', field: fields[0]?.name ?? '', value: fields[0] === undefined ? '' : typed(fields[0].kind, '') }])}>
            {intl.formatMessage(m.wsAddAssign)}
          </button>
        </div>
      ) : null}
    </fieldset>
  )
}

function TextField({ label: text, value, editable, onChange }: { label: string; value: string; editable: boolean; onChange: (value: string) => void }) {
  const id = useId()
  return (
    <label className={label} htmlFor={id}>
      {text}
      <input id={id} className={input} value={value} disabled={!editable} onChange={(e) => onChange(e.target.value)} />
    </label>
  )
}

function SelectField({ label: text, value, options, editable, onChange }: { label: string; value: string; options: { value: string; text: string }[]; editable: boolean; onChange: (value: string) => void }) {
  const id = useId()
  return (
    <label className={label} htmlFor={id}>
      {text}
      <select id={id} className={input} value={value} disabled={!editable} onChange={(e) => onChange(e.target.value)}>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.text}
          </option>
        ))}
      </select>
    </label>
  )
}
