import { css, cx } from '@health/design-system/css'
import { useMutation } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { sandboxMessages as m } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button, control } from '../../reception/styles'
import { StudioError, saveFile, type StudioPackagePage } from '../api'
import { refusalText } from '../wording'
import type { StudioSandbox } from './api'
import { tableOutcomes, type Value, type WorkflowDefinition } from './model'
import { OUTCOMES, answer, current, moveText, restart, scenarioFile, scenarioPath, send, setGate, setTableOutcome, undo, untaken, wait, type Outcome, type Run } from './sandbox'

const subheading = css({ fontSize: 'md', fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1' })
const input = cx(control, css({ width: '100%' }))
const row = css({ display: 'flex', alignItems: 'center', gap: '1', flexWrap: 'wrap' })
const list = css({ listStyle: 'disc', pl: '5', display: 'flex', flexDirection: 'column', gap: '0.5', fontSize: 'sm', overflowWrap: 'anywhere' })
const group = css({ display: 'flex', flexDirection: 'column', gap: '2', border: 'none', p: '0', m: '0' })
const legend = css({ fontWeight: 'semibold', mb: '1' })
const small = css({
  minHeight: 'control',
  minWidth: 'control',
  px: '2',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  overflowWrap: 'anywhere',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})

const failure = (intl: IntlShape, error: unknown) =>
  error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail || String(error.status) : refusalText(intl, error.refusal)) : String(error)

export interface SandboxPanelProps {
  /** The package version, `name@version`. */
  id: string
  /** The draft definition, for the outcomes each decision table can give. */
  definition: WorkflowDefinition
  run: Run
  /** The server's answer for `run`; absent until it comes. */
  played: StudioSandbox | undefined
  playing: boolean
  error: unknown
  editable: boolean
  /** The draft's revision, for saving a scenario. */
  revision: number
  /** The draft's files, so a scenario gets a path of its own. */
  files: string[]
  onRun: (run: Run) => void
  /** A scenario was saved: the page as it is now. */
  onSaved: (page: StudioPackagePage) => void
}

/**
 * The Workflow Studio's Sandbox mode (studio.md § The Workflow Studio; #499). Every step comes
 * from central, which plays the draft with every action stubbed: the panel shows the guard
 * answers, the outcome of each decision table a guard names, the gates' outcomes, what holds after the last step (configuration, stubbed actions,
 * open tasks, pending timers), the events it accepts and the history, and sends the author's
 * moves. *Save as scenario* writes the run the server returned into `tests/` through the
 * `If-Match` path.
 */
export function SandboxPanel({ id, definition, run, played, playing, error, editable, revision, files, onRun, onSaved }: SandboxPanelProps) {
  const intl = useIntl()
  const platform = usePlatform()
  const ids = { wait: useId(), name: useId() }
  const [by, setBy] = useState('PT1H')
  const [name, setName] = useState('')
  const now = current(played)
  const waiting = played === undefined ? 0 : untaken(run, played)
  // A move the server has not taken must be answered or undone before the next.
  const stuck = played === undefined || waiting > 0 || playing
  const save = useMutation({
    mutationFn: async () => {
      if (played === undefined) throw new Error('nothing played')
      const path = scenarioPath(name, new Set(files))
      const page = await saveFile(platform.transport, id, path, scenarioFile(played, name), revision)
      return { page, path }
    },
    onSuccess: ({ page }) => {
      onSaved(page)
      setName('')
    },
  })
  const unanswered = new Set(played?.unanswered ?? [])
  const unansweredTables = new Set(played?.unansweredTables ?? [])
  const joined = (items: readonly string[]) => (items.length === 0 ? intl.formatMessage(m.sbNone) : items.join(', '))

  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <p className={muted}>{intl.formatMessage(m.sbHint)}</p>
      {error === null || error === undefined ? null : (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.sbFailed, { reason: failure(intl, error) })}
        </p>
      )}
      {played === undefined ? <p role="status">{intl.formatMessage(m.sbPlaying)}</p> : null}
      {played !== undefined && played.unanswered.length > 0 ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.sbUnanswered, { guards: played.unanswered.join(', ') })}
        </p>
      ) : null}
      {played !== undefined && played.unansweredTables.length > 0 ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.sbUnansweredTables, { tables: played.unansweredTables.join(', ') })}
        </p>
      ) : null}
      {played?.refused === null || played?.refused === undefined ? null : (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.sbRefused, { reason: played.refused })}
        </p>
      )}

      {played === undefined ? null : (
        <fieldset className={group}>
          <legend className={legend}>{intl.formatMessage(m.sbGuards)}</legend>
          {played.guards.length === 0 ? <p className={muted}>{intl.formatMessage(m.sbGuardsNone)}</p> : null}
          {played.guards.map((guard) => (
            <GuardAnswer key={guard} guard={guard} value={run.guards[guard]} flagged={unanswered.has(guard)} onChange={(holds) => onRun(answer(run, guard, holds))} />
          ))}
        </fieldset>
      )}
      {played === undefined || played.tables.length === 0 ? null : (
        <fieldset className={group}>
          <legend className={legend}>{intl.formatMessage(m.sbTables)}</legend>
          <p className={muted}>{intl.formatMessage(m.sbTablesHint)}</p>
          {played.tables.map((table) => (
            <TableOutcome
              key={table}
              table={table}
              outcomes={(definition.decisionTables ?? []).filter((t) => t.id === table).flatMap(tableOutcomes)}
              value={run.tables[table]}
              flagged={unansweredTables.has(table)}
              onChange={(outcome) => onRun(setTableOutcome(run, table, outcome))}
            />
          ))}
        </fieldset>
      )}
      {played === undefined || played.gates.length === 0 ? null : (
        <fieldset className={group}>
          <legend className={legend}>{intl.formatMessage(m.sbGates)}</legend>
          {played.gates.map((gate) => (
            <GateOutcome key={gate} gate={gate} value={run.gates[gate]} onChange={(outcome) => onRun(setGate(run, gate, outcome))} />
          ))}
        </fieldset>
      )}

      {now === undefined || played === undefined ? null : (
        <section aria-labelledby="sb-now" className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          <h3 id="sb-now" className={subheading}>
            {intl.formatMessage(m.sbNow, { step: played.steps.length - 1 })}
          </h3>
          <p className={muted}>{intl.formatMessage(m.sbElapsed, { elapsed: now.elapsed })}</p>
          {now.running ? null : <p className={muted}>{intl.formatMessage(m.sbCompleted)}</p>}
          <dl className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm', '& dt': { fontWeight: 'semibold' }, '& dd': { overflowWrap: 'anywhere' } })}>
            <dt>{intl.formatMessage(m.sbConfiguration)}</dt>
            <dd>{joined(now.configuration)}</dd>
            <dt>{intl.formatMessage(m.sbActions)}</dt>
            <dd>{joined(now.actions)}</dd>
            <dt>{intl.formatMessage(m.sbTasks)}</dt>
            <dd>{joined(now.tasks.map((t) => intl.formatMessage(m.sbTask, { state: t.state, status: t.status })))}</dd>
            <dt>{intl.formatMessage(m.sbTimers)}</dt>
            <dd>{joined(now.timers.map((t) => intl.formatMessage(m.sbTimer, { event: t.event, dueIn: t.dueIn })))}</dd>
          </dl>

          <h3 className={subheading}>{intl.formatMessage(m.sbEvents)}</h3>
          {now.accepts.length === 0 || !now.running ? <p className={muted}>{intl.formatMessage(m.sbEventsNone)}</p> : null}
          <div className={row}>
            {now.running
              ? now.accepts.map((event) => (
                  <button key={event} type="button" className={small} disabled={stuck} onClick={() => onRun(send(run, event))}>
                    {intl.formatMessage(m.sbSend, { event })}
                  </button>
                ))
              : null}
            {now.running
              ? now.timers.map((t, n) => (
                  <button key={`${t.event}:${n}`} type="button" className={small} disabled={stuck} onClick={() => onRun(wait(run, t.dueIn))}>
                    {intl.formatMessage(m.sbWaitFor, { event: t.event, dueIn: t.dueIn })}
                  </button>
                ))
              : null}
          </div>
          <div className={css({ display: 'flex', gap: '1', alignItems: 'flex-end' })}>
            <label className={cx(label, css({ flex: '1' }))} htmlFor={ids.wait}>
              {intl.formatMessage(m.sbWait)}
              <input id={ids.wait} className={input} value={by} onChange={(e) => setBy(e.target.value)} />
            </label>
            <button type="button" className={small} disabled={stuck || !now.running || by.trim() === ''} onClick={() => onRun(wait(run, by.trim()))}>
              {intl.formatMessage(m.sbWaitButton)}
            </button>
          </div>
        </section>
      )}
      <div className={row}>
        <button type="button" className={small} disabled={run.moves.length === 0 || playing} onClick={() => onRun(undo(run))}>
          {intl.formatMessage(m.sbUndo)}
        </button>
        <button type="button" className={small} disabled={run.moves.length === 0 || playing} onClick={() => onRun(restart(run))}>
          {intl.formatMessage(m.sbRestart)}
        </button>
      </div>

      {played === undefined ? null : <History run={run} played={played} />}

      {editable && played !== undefined ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          <label className={label} htmlFor={ids.name}>
            {intl.formatMessage(m.sbScenarioName)}
            <input id={ids.name} className={input} value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <button type="button" className={button} disabled={name.trim() === '' || stuck || played.steps.length === 0 || save.isPending} onClick={() => save.mutate()}>
            {intl.formatMessage(m.sbSave)}
          </button>
        </div>
      ) : null}
      {save.isSuccess ? <p role="status">{intl.formatMessage(m.sbSaved, { path: save.data.path })}</p> : null}
      {save.error === null ? null : (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.sbSaveFailed, { reason: failure(intl, save.error) })}
        </p>
      )}
    </div>
  )
}

function GuardAnswer({ guard, value, flagged, onChange }: { guard: string; value: boolean | undefined; flagged: boolean; onChange: (holds: boolean | null) => void }) {
  const intl = useIntl()
  const field = useId()
  return (
    <label className={label} htmlFor={field}>
      <span className={cx(css({ overflowWrap: 'anywhere', fontFamily: 'mono', fontSize: 'sm' }), flagged ? alert : undefined)}>{intl.formatMessage(m.sbAnswer, { guard })}</span>
      <select
        id={field}
        className={input}
        aria-invalid={flagged ? true : undefined}
        value={value === undefined ? '' : String(value)}
        onChange={(e) => onChange(e.target.value === '' ? null : e.target.value === 'true')}
      >
        {(['', 'true', 'false'] as const).map((v) => (
          <option key={v} value={v}>
            {intl.formatMessage(m.sbAnswerName, { answer: v === '' ? 'none' : v })}
          </option>
        ))}
      </select>
    </label>
  )
}

function TableOutcome({ table, outcomes, value, flagged, onChange }: { table: string; outcomes: Value[]; value: Value | undefined; flagged: boolean; onChange: (outcome: Value | null) => void }) {
  const intl = useIntl()
  const field = useId()
  return (
    <label className={label} htmlFor={field}>
      <span className={cx(css({ overflowWrap: 'anywhere', fontSize: 'sm' }), flagged ? alert : undefined)}>{intl.formatMessage(m.sbTable, { table })}</span>
      <select
        id={field}
        className={input}
        aria-invalid={flagged ? true : undefined}
        value={value === undefined ? '' : String(value)}
        onChange={(e) => onChange(outcomes.find((o) => String(o) === e.target.value) ?? null)}
      >
        <option value="">{intl.formatMessage(m.sbAnswerName, { answer: 'none' })}</option>
        {outcomes.map((o) => (
          <option key={String(o)} value={String(o)}>
            {String(o)}
          </option>
        ))}
      </select>
    </label>
  )
}

function GateOutcome({ gate, value, onChange }: { gate: string; value: string | undefined; onChange: (outcome: Outcome | null) => void }) {
  const intl = useIntl()
  const field = useId()
  return (
    <label className={label} htmlFor={field}>
      <span className={css({ overflowWrap: 'anywhere', fontSize: 'sm' })}>{intl.formatMessage(m.sbGate, { gate })}</span>
      <select id={field} className={input} value={value ?? 'defer'} onChange={(e) => onChange(e.target.value === 'defer' ? null : (e.target.value as Outcome))}>
        {OUTCOMES.map((o) => (
          <option key={o} value={o}>
            {intl.formatMessage(m.sbOutcomeName, { outcome: o })}
          </option>
        ))}
      </select>
    </label>
  )
}

/** The run so far: the start, each move taken with the configuration it reached, and the moves
 * not taken. */
function History({ run, played }: { run: Run; played: StudioSandbox }) {
  const intl = useIntl()
  const [start, ...after] = played.steps
  return (
    <section aria-labelledby="sb-history" className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <h3 id="sb-history" className={subheading}>
        {intl.formatMessage(m.sbHistory)}
      </h3>
      <ol className={cx(list, css({ listStyle: 'decimal' }))}>
        {start === undefined ? null : <li>{intl.formatMessage(m.sbHistoryStart, { configuration: start.configuration.join(', ') })}</li>}
        {run.moves.map((move, n) => {
          const what = moveText(move)
          const values = { kind: what.kind, event: what.kind === 'event' ? what.event : '', wait: what.kind === 'wait' ? what.wait : '' }
          const reached = after[n]
          return (
            <li key={n}>
              {reached === undefined || start === undefined
                ? intl.formatMessage(m.sbHistoryNotTaken, values)
                : intl.formatMessage(m.sbHistoryMove, { ...values, configuration: reached.configuration.join(', ') })}
            </li>
          )
        })}
      </ol>
    </section>
  )
}
