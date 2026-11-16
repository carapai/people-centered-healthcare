// PROTOTYPE (#481), throwaway: three structurally different Workflow Studios on
// /prototype/workflow-studio, switched with ?variant=A|B|C and the floating bar (← / → keys).
// A: canvas and inspector, with the package's scenarios replayed on the canvas step by step.
// B: an outline of the nesting beside the canvas, and a free-play sandbox that saves what you
//    played as a new scenario. C: the definition as two tables (states, transitions), a small map,
//    and the scenarios as an expected-against-actual grid.
// All three edit one in-memory Workflow Definition; "simulation" is a TypeScript stand-in for the
// Rust engine's sandbox (stubbed guards and actions), good enough to judge the screens.
import { css, cx } from '@health/design-system/css'
import { useEffect, useMemo, useState } from 'react'

import { WorkflowCanvas } from './WorkflowCanvas'
import {
  ACTIONS,
  FORMS,
  GATES,
  ROLES,
  SAMPLE,
  SCENARIOS,
  SUBWORKFLOWS,
  checks,
  describeAction,
  guardKey,
  guardText,
  simulate,
  type Action,
  type Check,
  type Definition,
  type Guard,
  type Kind,
  type Scenario,
  type State,
  type Trace,
  type Transition,
} from './workflowData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', width: '100%', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })
const legend = css({ fontWeight: 'semibold', mb: '1' })
const fieldset = css({ display: 'flex', flexDirection: 'column', gap: '2', borderTop: '1px solid', borderColor: 'border', pt: '2' })
const ok = css({ color: 'success.text' })
const bad = css({ color: 'danger.text' })

// --- The model -----------------------------------------------------------------------------

function useDefinition() {
  const [def, setDef] = useState<Definition>(SAMPLE)
  const [scenarios, setScenarios] = useState<Scenario[]>(SCENARIOS)
  const [revision, setRevision] = useState(14)
  const change = (f: (d: Definition) => Definition) => {
    setDef(f)
    setRevision((r) => r + 1)
  }
  const editState = (id: string, patch: Partial<State>) => change((d) => ({ ...d, states: d.states.map((x) => (x.id === id ? { ...x, ...patch } : x)) }))
  const editTransition = (id: string, patch: Partial<Transition>) => change((d) => ({ ...d, transitions: d.transitions.map((x) => (x.id === id ? { ...x, ...patch } : x)) }))
  const addState = (parent: string | null) => {
    const id = `state-${Date.now().toString(36)}`
    change((d) => ({ ...d, states: [...d.states, { id, label: 'New state', kind: 'atomic', parent, entry: [] }] }))
    return id
  }
  const addTransition = (from: string, to: string) => {
    const id = `t-${Date.now().toString(36)}`
    change((d) => ({ ...d, transitions: [...d.transitions, { id, from, to, event: 'new.event', guard: { kind: 'none' }, actions: [] }] }))
    return id
  }
  const remove = (id: string) =>
    change((d) => {
      const gone = new Set([id])
      for (let grew = true; grew; ) {
        grew = false
        for (const x of d.states) {
          if (x.parent && gone.has(x.parent) && !gone.has(x.id)) {
            gone.add(x.id)
            grew = true
          }
        }
      }
      return { ...d, states: d.states.filter((x) => !gone.has(x.id)), transitions: d.transitions.filter((x) => x.id !== id && !gone.has(x.from) && !gone.has(x.to)) }
    })
  const editTable = (id: string, rows: { when: string; outcome: string }[]) => change((d) => ({ ...d, tables: d.tables.map((t) => (t.id === id ? { ...t, rows } : t)) }))
  const grant = (permission: string) => change((d) => ({ ...d, principal: d.principal.includes(permission) ? d.principal : [...d.principal, permission] }))
  return { def, scenarios, setScenarios, revision, editState, editTransition, addState, addTransition, remove, editTable, grant }
}
type Model = ReturnType<typeof useDefinition>

const stateLabel = (def: Definition, id: string) => def.states.find((x) => x.id === id)?.label ?? id
const flaggedIds = (found: Check[]) => found.flatMap((c) => c.problems.flatMap((p) => [p.state, p.transition].filter((x): x is string => x !== undefined)))

// --- Shared pieces -------------------------------------------------------------------------

/** Typed actions, each with the parameters its generated schema asks for. */
function ActionsEditor({ actions, onChange }: { actions: Action[]; onChange: (a: Action[]) => void }) {
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      {actions.map((a, n) => {
        const spec = ACTIONS.find((x) => x.id === a.id)
        const set = (next: Action) => onChange(actions.map((x, m) => (m === n ? next : x)))
        return (
          <div key={`${a.id}-${n}`} className={cx(box, css({ p: '2', display: 'flex', flexDirection: 'column', gap: '1' }))}>
            <div className={css({ display: 'flex', gap: '2' })}>
              <select aria-label="Action" className={input} value={a.id} onChange={(e) => set({ id: e.target.value, params: {} })}>
                {ACTIONS.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.title} ({x.id})
                  </option>
                ))}
              </select>
              <button type="button" className={ghost} aria-label="Remove action" onClick={() => onChange(actions.filter((_, m) => m !== n))}>
                ✕
              </button>
            </div>
            {spec ? (
              <span className={muted}>
                Needs {spec.permission} · undo: {spec.compensation}
              </span>
            ) : null}
            {spec?.params.map((p) => (
              <label key={p.name} className={label}>
                {p.name}
                {p.required ? ' *' : ''}
                {p.values ? (
                  <select className={input} value={a.params[p.name] ?? ''} onChange={(e) => set({ ...a, params: { ...a.params, [p.name]: e.target.value } })}>
                    <option value="">—</option>
                    {p.values.map((v) => (
                      <option key={v}>{v}</option>
                    ))}
                  </select>
                ) : (
                  <input className={input} value={a.params[p.name] ?? ''} onChange={(e) => set({ ...a, params: { ...a.params, [p.name]: e.target.value } })} />
                )}
              </label>
            ))}
          </div>
        )
      })}
      <button type="button" className={ghost} onClick={() => onChange([...actions, { id: ACTIONS[0]?.id ?? '', params: {} }])}>
        Add action
      </button>
    </div>
  )
}

function GuardEditor({ model, tr }: { model: Model; tr: Transition }) {
  const g = tr.guard
  const setGuard = (guard: Guard) => model.editTransition(tr.id, { guard })
  const table = g.kind === 'table' ? model.def.tables.find((t) => t.id === g.table) : undefined
  return (
    <>
      <label className={label}>
        Guard
        <select className={input} value={g.kind} onChange={(e) => setGuard(e.target.value === 'fhirpath' ? { kind: 'fhirpath', expression: '' } : e.target.value === 'table' ? { kind: 'table', table: model.def.tables[0]?.id ?? '', outcome: '' } : { kind: 'none' })}>
          <option value="none">Always</option>
          <option value="fhirpath">When a FHIRPath expression is true</option>
          <option value="table">When a decision table gives an outcome</option>
        </select>
      </label>
      {g.kind === 'fhirpath' ? (
        <label className={label}>
          FHIRPath <span className={muted}>May read: {model.def.dataRequirements.join(', ')}</span>
          <textarea className={cx(input, css({ fontFamily: 'mono', minHeight: '4rem', py: '1' }))} value={g.expression} onChange={(e) => setGuard({ ...g, expression: e.target.value })} />
        </label>
      ) : null}
      {g.kind === 'table' ? (
        <>
          <label className={label}>
            Table
            <select className={input} value={g.table} onChange={(e) => setGuard({ ...g, table: e.target.value })}>
              {model.def.tables.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.title}
                </option>
              ))}
            </select>
          </label>
          {table ? (
            <table className={css({ fontSize: 'xs', width: '100%', '& td': { p: '1', verticalAlign: 'top' } })}>
              <thead>
                <tr>
                  <th align="left">When (first match wins)</th>
                  <th align="left">Outcome</th>
                </tr>
              </thead>
              <tbody>
                {table.rows.map((r, n) => (
                  <tr key={n}>
                    <td>
                      <input className={cx(input, css({ fontFamily: 'mono', fontSize: 'xs' }))} value={r.when} onChange={(e) => model.editTable(table.id, table.rows.map((x, m) => (m === n ? { ...x, when: e.target.value } : x)))} />
                    </td>
                    <td>
                      <input className={cx(input, css({ fontSize: 'xs' }))} value={r.outcome} onChange={(e) => model.editTable(table.id, table.rows.map((x, m) => (m === n ? { ...x, outcome: e.target.value } : x)))} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}
          <label className={label}>
            Take this transition when the outcome is
            <select className={input} value={g.outcome} onChange={(e) => setGuard({ ...g, outcome: e.target.value })}>
              <option value="">—</option>
              {[...new Set(table?.rows.map((r) => r.outcome) ?? [])].map((o) => (
                <option key={o}>{o}</option>
              ))}
            </select>
          </label>
        </>
      ) : null}
    </>
  )
}

/** Every property of the selected state or transition, as one panel of ordinary form fields. */
function Inspector({ model, selected, onSelect }: { model: Model; selected: string | null; onSelect: (id: string | null) => void }) {
  const { def } = model
  const st = def.states.find((x) => x.id === selected)
  const tr = def.transitions.find((x) => x.id === selected)
  if (!st && !tr) {
    return (
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <p className={muted}>Select a state or a transition. Drag from a state's bottom handle to another state to add a transition.</p>
        <button type="button" className={ghost} onClick={() => onSelect(model.addState(null))}>
          Add state
        </button>
        <fieldset className={fieldset}>
          <legend className={legend}>Definition</legend>
          <span className={muted}>
            {def.url}|{def.version}
          </span>
          <span className={css({ fontSize: 'sm' })}>Data the guards may read: {def.dataRequirements.join(', ')}</span>
          <span className={css({ fontSize: 'sm' })}>Workflow principal (timers, eventless steps): {def.principal.join(', ')}</span>
        </fieldset>
      </div>
    )
  }
  if (st) {
    const containers = def.states.filter((x) => (x.kind === 'compound' || x.kind === 'parallel') && x.id !== st.id)
    const kids = def.states.filter((x) => x.parent === st.id)
    return (
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          <legend className={legend}>State</legend>
          <label className={label}>
            Name
            <input className={input} value={st.label} onChange={(e) => model.editState(st.id, { label: e.target.value })} />
          </label>
          <label className={label}>
            Id <span className={muted}>Fixed once published: running instances and the migration map refer to it</span>
            <input className={input} value={st.id} readOnly />
          </label>
          <label className={label}>
            Kind
            <select className={input} value={st.kind} onChange={(e) => model.editState(st.id, { kind: e.target.value as Kind })}>
              <option value="atomic">Atomic: one step</option>
              <option value="compound">Compound: one of its children at a time</option>
              <option value="parallel">Parallel: all of its regions at once</option>
              <option value="final">Final: the end of its parent</option>
            </select>
          </label>
          <label className={label}>
            Inside
            <select className={input} value={st.parent ?? ''} onChange={(e) => model.editState(st.id, { parent: e.target.value || null })}>
              <option value="">(top level)</option>
              {containers.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.label}
                </option>
              ))}
            </select>
          </label>
          {st.kind === 'compound' ? (
            <label className={label}>
              Starts in
              <select className={input} value={st.initial ?? ''} onChange={(e) => model.editState(st.id, { initial: e.target.value })}>
                {kids.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.label}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
          {st.kind === 'compound' || st.kind === 'parallel' ? (
            <button type="button" className={ghost} onClick={() => onSelect(model.addState(st.id))}>
              Add a state inside
            </button>
          ) : null}
        </fieldset>
        {st.kind === 'atomic' ? (
          <fieldset className={fieldset}>
            <legend className={legend}>What happens here</legend>
            <select
              aria-label="What happens here"
              className={input}
              value={st.task ? 'task' : st.gate ? 'gate' : st.invoke ? 'invoke' : 'wait'}
              onChange={(e) => {
                const v = e.target.value
                model.editState(st.id, {
                  task: v === 'task' ? { role: ROLES[0] ?? '', worklist: '', form: '', priority: 'routine', claimTimeout: 'PT15M' } : undefined,
                  gate: v === 'gate' ? GATES[0] : undefined,
                  invoke: v === 'invoke' ? SUBWORKFLOWS[0] : undefined,
                })
              }}
            >
              <option value="wait">Wait for an event</option>
              <option value="task">A person does a task</option>
              <option value="gate">A payment gate</option>
              <option value="invoke">Run a sub-workflow</option>
            </select>
            {st.task ? (
              <>
                <label className={label}>
                  Role
                  <select className={input} value={st.task.role} onChange={(e) => st.task && model.editState(st.id, { task: { ...st.task, role: e.target.value } })}>
                    {ROLES.map((r) => (
                      <option key={r}>{r}</option>
                    ))}
                  </select>
                </label>
                <label className={label}>
                  Worklist
                  <input className={input} value={st.task.worklist} onChange={(e) => st.task && model.editState(st.id, { task: { ...st.task, worklist: e.target.value } })} />
                </label>
                <label className={label}>
                  Form <span className={muted}>A Questionnaire url|version from this package or an active one</span>
                  <select className={input} value={st.task.form} onChange={(e) => st.task && model.editState(st.id, { task: { ...st.task, form: e.target.value } })}>
                    {FORMS.map((f) => (
                      <option key={f} value={f}>
                        {f === '' ? '(no form)' : f.split('/').pop()}
                      </option>
                    ))}
                  </select>
                </label>
                <div className={css({ display: 'flex', gap: '2' })}>
                  <label className={label}>
                    Priority
                    <select className={input} value={st.task.priority} onChange={(e) => st.task && model.editState(st.id, { task: { ...st.task, priority: e.target.value as 'routine' } })}>
                      <option>routine</option>
                      <option>urgent</option>
                      <option>emergency</option>
                    </select>
                  </label>
                  <label className={label}>
                    Claim timeout
                    <input className={input} value={st.task.claimTimeout} onChange={(e) => st.task && model.editState(st.id, { task: { ...st.task, claimTimeout: e.target.value } })} />
                  </label>
                </div>
              </>
            ) : null}
            {st.gate !== undefined ? (
              <label className={label}>
                Gate configuration <span className={muted}>Leaves on gate.pass, gate.defer or gate.block; facilities switch it on or off</span>
                <select className={input} value={st.gate} onChange={(e) => model.editState(st.id, { gate: e.target.value })}>
                  {GATES.map((g) => (
                    <option key={g}>{g}</option>
                  ))}
                </select>
              </label>
            ) : null}
            {st.invoke !== undefined ? (
              <label className={label}>
                Sub-workflow <span className={muted}>Sends done.invoke.{st.id} (or error.invoke.{st.id}) when it ends</span>
                <select className={input} value={st.invoke} onChange={(e) => model.editState(st.id, { invoke: e.target.value })}>
                  {SUBWORKFLOWS.map((w) => (
                    <option key={w} value={w}>
                      {w.split('/').pop()}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
          </fieldset>
        ) : null}
        <fieldset className={fieldset}>
          <legend className={legend}>On entry</legend>
          <ActionsEditor actions={st.entry} onChange={(entry) => model.editState(st.id, { entry })} />
        </fieldset>
        <fieldset className={fieldset}>
          <legend className={legend}>Leaves by</legend>
          {def.transitions
            .filter((x) => x.from === st.id)
            .map((x) => (
              <button key={x.id} type="button" className={cx(ghost, css({ textAlign: 'left' }))} onClick={() => onSelect(x.id)}>
                {x.event || '(eventless)'} {guardText(x.guard)} → {stateLabel(def, x.to)}
              </button>
            ))}
        </fieldset>
        <button type="button" className={cx(ghost, bad)} onClick={() => (model.remove(st.id), onSelect(null))}>
          Delete state{kids.length ? ' and what is inside it' : ''}
        </button>
      </div>
    )
  }
  if (!tr) return null
  const timer = tr.event.startsWith('after ')
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <legend className={legend}>Transition</legend>
        <div className={css({ display: 'flex', gap: '2' })}>
          <label className={label}>
            From
            <select className={input} value={tr.from} onChange={(e) => model.editTransition(tr.id, { from: e.target.value })}>
              {def.states.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.label}
                </option>
              ))}
            </select>
          </label>
          <label className={label}>
            To
            <select className={input} value={tr.to} onChange={(e) => model.editTransition(tr.id, { to: e.target.value })}>
              {def.states.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.label}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label className={label}>
          Triggered by
          <select
            className={input}
            value={timer ? 'timer' : tr.event === '' ? 'eventless' : 'event'}
            onChange={(e) => model.editTransition(tr.id, { event: e.target.value === 'timer' ? 'after PT30M' : e.target.value === 'eventless' ? '' : 'new.event' })}
          >
            <option value="event">An event</option>
            <option value="timer">A timer</option>
            <option value="eventless">Nothing: as soon as the guard holds</option>
          </select>
        </label>
        {timer ? (
          <label className={label}>
            After (ISO 8601 duration) <span className={muted}>Runs as the workflow principal, not as a person</span>
            <input className={input} value={tr.event.slice(6)} onChange={(e) => model.editTransition(tr.id, { event: `after ${e.target.value}` })} />
          </label>
        ) : tr.event !== '' ? (
          <label className={label}>
            Event
            <input className={cx(input, css({ fontFamily: 'mono' }))} value={tr.event} onChange={(e) => model.editTransition(tr.id, { event: e.target.value })} />
          </label>
        ) : null}
        <GuardEditor model={model} tr={tr} />
        <span className={muted}>When several transitions leave on the same event, the first one whose guard holds wins, in this order:</span>
        <ol className={css({ fontSize: 'sm', pl: '5', listStyle: 'decimal' })}>
          {def.transitions
            .filter((x) => x.from === tr.from && x.event === tr.event)
            .map((x) => (
              <li key={x.id} className={x.id === tr.id ? css({ fontWeight: 'semibold' }) : undefined}>
                {guardText(x.guard) || '[always]'} → {stateLabel(def, x.to)}
              </li>
            ))}
        </ol>
      </fieldset>
      <fieldset className={fieldset}>
        <legend className={legend}>Actions</legend>
        <ActionsEditor actions={tr.actions} onChange={(actions) => model.editTransition(tr.id, { actions })} />
      </fieldset>
      <button type="button" className={cx(ghost, bad)} onClick={() => (model.remove(tr.id), onSelect(null))}>
        Delete transition
      </button>
    </div>
  )
}

function ChecksPanel({ found, onSelect, onGrant, open = false }: { found: Check[]; onSelect: (id: string) => void; onGrant?: (permission: string) => void; open?: boolean }) {
  const failing = found.filter((c) => c.problems.length > 0).length
  return (
    <details open={open || failing > 0} className={cx(box, css({ p: '3' }))}>
      <summary className={css({ cursor: 'pointer', fontWeight: 'semibold' })}>
        Publication checks: {failing === 0 ? <span className={ok}>all pass</span> : <span className={bad}>{failing} failing</span>} <span className={muted}>(the same checks submission runs; Check now on the draft page decides)</span>
      </summary>
      <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', mt: '2', fontSize: 'sm' })}>
        {found.map((c) => (
          <li key={c.title}>
            {c.problems.length === 0 ? <span className={ok}>✓</span> : <span className={bad}>✗</span>} {c.title}
            <ul className={css({ pl: '5' })}>
              {c.problems.map((p) => {
                const permission = /needs ([a-z.-]+);/.exec(p.text)?.[1]
                return (
                  <li key={p.text}>
                    {p.state || p.transition ? (
                      <button type="button" className={css({ textDecoration: 'underline', cursor: 'pointer', textAlign: 'left' })} onClick={() => onSelect(p.state ?? p.transition ?? '')}>
                        {p.text}
                      </button>
                    ) : (
                      p.text
                    )}
                    {permission && onGrant ? (
                      <button type="button" className={cx(ghost, css({ ml: '2', minHeight: 'auto', py: '0' }))} onClick={() => onGrant(permission)}>
                        Declare {permission} for approval
                      </button>
                    ) : null}
                  </li>
                )
              })}
            </ul>
          </li>
        ))}
      </ul>
    </details>
  )
}

const StepFacts = ({ trace }: { trace: Trace }) => (
  <dl className={css({ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '1 3', fontSize: 'sm', '& dt': { color: 'fg.muted' } })}>
    <dt>Now in</dt>
    <dd>{trace.configuration.join(', ') || 'nothing (ended)'}</dd>
    <dt>Actions (stubbed)</dt>
    <dd>{trace.actions.length ? trace.actions.join('; ') : '—'}</dd>
    <dt>Open tasks</dt>
    <dd>{trace.tasks.length ? trace.tasks.join('; ') : '—'}</dd>
    <dt>Timers</dt>
    <dd>{trace.timers.length ? trace.timers.join('; ') : '—'}</dd>
    {trace.pass ? null : (
      <>
        <dt className={bad}>Expected</dt>
        <dd className={bad}>{trace.error ?? trace.expected.join(', ')}</dd>
      </>
    )}
  </dl>
)

// --- Variant A: canvas and inspector; scenarios replayed on the canvas ------------------------

function VariantA({ model }: { model: Model }) {
  const [selected, setSelected] = useState<string | null>(null)
  const [scenario, setScenario] = useState(0)
  const [step, setStep] = useState<number | null>(null)
  const found = useMemo(() => checks(model.def, model.scenarios), [model.def, model.scenarios])
  const sc = model.scenarios[scenario] ?? model.scenarios[0]
  const traces = useMemo(() => (sc ? simulate(model.def, sc) : []), [model.def, sc])
  const shown = step === null ? undefined : traces[step]
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <ChecksPanel found={found} onSelect={setSelected} onGrant={model.grant} />
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 22rem' }, gap: '3' })}>
        <div className={box}>
          <WorkflowCanvas
            def={model.def}
            selected={selected}
            onSelect={setSelected}
            active={shown?.configuration}
            taken={shown?.taken}
            flagged={shown ? [] : flaggedIds(found)}
            onConnect={(from, to) => setSelected(model.addTransition(from, to))}
            height={600}
          />
        </div>
        <aside className={cx(box, css({ p: '3', maxHeight: '600px', overflowY: 'auto' }))}>
          <Inspector model={model} selected={selected} onSelect={setSelected} />
        </aside>
      </div>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))} aria-label="Simulation">
        <div className={css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap' })}>
          <strong>Simulate</strong>
          <select aria-label="Scenario" className={cx(input, css({ width: 'auto' }))} value={scenario} onChange={(e) => (setScenario(Number(e.target.value)), setStep(0))}>
            {model.scenarios.map((x, n) => (
              <option key={x.name} value={n}>
                {simulate(model.def, x).every((t) => t.pass) ? '✓' : '✗'} {x.name}
              </option>
            ))}
          </select>
          <span className={muted}>Stubbed guards: {sc && Object.keys(sc.guards).length ? Object.entries(sc.guards).map(([k, v]) => `${k} → ${v}`).join('; ') : 'none'}</span>
          {step !== null ? (
            <button type="button" className={ghost} onClick={() => setStep(null)}>
              Back to editing
            </button>
          ) : null}
        </div>
        <ol className={css({ display: 'flex', gap: '1', flexWrap: 'wrap' })}>
          {traces.map((t, n) => (
            <li key={n}>
              <button
                type="button"
                aria-pressed={step === n}
                onClick={() => setStep(n)}
                className={cx(ghost, css({ fontSize: 'sm' }), step === n ? css({ bg: 'bg.selected', borderColor: 'accent.default' }) : undefined, t.pass ? undefined : css({ borderColor: 'danger.text', color: 'danger.text' }))}
              >
                {n + 1}. {t.label} {t.pass ? '' : '✗'}
              </button>
            </li>
          ))}
        </ol>
        {shown ? <StepFacts trace={shown} /> : <p className={muted}>Pick a step to see where the instance is; the path it took lights up on the canvas.</p>}
      </section>
    </div>
  )
}

// --- Variant B: outline beside the canvas; a free-play sandbox -----------------------------------

function Outline({ model, selected, onSelect, active }: { model: Model; selected: string | null; onSelect: (id: string) => void; active: string[] }) {
  const { def } = model
  const glyph: Record<Kind, string> = { atomic: '○', compound: '▣', parallel: '⫴', final: '◉' }
  const branch = (parent: string | null, depth: number): React.ReactNode =>
    def.states
      .filter((x) => x.parent === parent)
      .map((x) => (
        <li key={x.id}>
          <button
            type="button"
            onClick={() => onSelect(x.id)}
            className={cx(css({ width: '100%', textAlign: 'left', px: '2', py: '1', borderRadius: 'sm', cursor: 'pointer', fontSize: 'sm' }), selected === x.id ? css({ bg: 'bg.selected' }) : undefined, active.includes(x.id) ? css({ fontWeight: 'bold', color: 'accent.default' }) : undefined)}
            style={{ paddingLeft: `${0.5 + depth}rem` }}
          >
            {glyph[x.kind]} {x.label}
            {x.task ? <span className={muted}> · {x.task.role}</span> : null}
            {x.gate ? <span className={muted}> · gate</span> : null}
            {x.invoke ? <span className={muted}> · sub-workflow</span> : null}
          </button>
          <ul>
            {def.transitions
              .filter((t) => t.from === x.id)
              .map((t) => (
                <li key={t.id}>
                  <button
                    type="button"
                    onClick={() => onSelect(t.id)}
                    className={cx(css({ width: '100%', textAlign: 'left', py: '0.5', cursor: 'pointer', fontSize: 'xs', color: 'fg.muted' }), selected === t.id ? css({ bg: 'bg.selected' }) : undefined)}
                    style={{ paddingLeft: `${1.5 + depth}rem` }}
                  >
                    {t.event.startsWith('after ') ? '⏱ ' : '↳ '}
                    {t.event || '(eventless)'} {guardText(t.guard)} → {stateLabel(def, t.to)}
                  </button>
                </li>
              ))}
          </ul>
          <ul>{branch(x.id, depth + 1)}</ul>
        </li>
      ))
  return <ul>{branch(null, 0)}</ul>
}

function Sandbox({ model, onTrace }: { model: Model; onTrace: (t: Trace | undefined) => void }) {
  const { def } = model
  const [played, setPlayed] = useState<{ event?: string; wait?: string }[]>([])
  const [stubs, setStubs] = useState<Record<string, boolean>>({})
  const [name, setName] = useState('')
  const guards = [...new Set(def.transitions.map((t) => guardKey(t.guard)).filter(Boolean))]
  const scenario: Scenario = { name: 'sandbox', guards: Object.fromEntries(guards.map((g) => [g, stubs[g] ?? false])), steps: [{ expect: [] }, ...played.map((p) => ({ ...p, expect: [] }))] }
  const traces = simulate(def, scenario)
  const now = traces[traces.length - 1]
  useEffect(() => onTrace(now), [JSON.stringify(now)]) // eslint-disable-line react-hooks/exhaustive-deps
  const activeSet = new Set(now?.configuration ?? [])
  const ancestorsActive = (id: string) => activeSet.has(id)
  const events = [...new Set(def.transitions.filter((t) => ancestorsActive(t.from) && t.event !== '').map((t) => t.event))]
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <p className={muted}>Play events against the draft, as the engine's sandbox would, with every action stubbed.</p>
      <fieldset className={fieldset}>
        <legend className={legend}>Guards answer</legend>
        {guards.map((g) => (
          <label key={g} className={css({ fontSize: 'sm', fontFamily: 'mono' })}>
            <input type="checkbox" checked={stubs[g] ?? false} onChange={(e) => setStubs({ ...stubs, [g]: e.target.checked })} /> {g}
          </label>
        ))}
      </fieldset>
      <fieldset className={fieldset}>
        <legend className={legend}>Send</legend>
        <div className={css({ display: 'flex', gap: '1', flexWrap: 'wrap' })}>
          {events.map((e) => (
            <button key={e} type="button" className={cx(ghost, css({ fontFamily: 'mono', fontSize: 'xs' }))} onClick={() => setPlayed([...played, e.startsWith('after ') ? { wait: e.slice(6) } : { event: e }])}>
              {e.startsWith('after ') ? `wait ${e.slice(6)}` : e}
            </button>
          ))}
          {events.length === 0 ? <span className={muted}>Nothing can happen from here.</span> : null}
        </div>
        <div className={css({ display: 'flex', gap: '2' })}>
          <button type="button" className={ghost} disabled={played.length === 0} onClick={() => setPlayed(played.slice(0, -1))}>
            Undo
          </button>
          <button type="button" className={ghost} onClick={() => setPlayed([])}>
            Restart
          </button>
        </div>
      </fieldset>
      {now ? <StepFacts trace={{ ...now, pass: true }} /> : null}
      <fieldset className={fieldset}>
        <legend className={legend}>History</legend>
        <ol className={css({ fontSize: 'sm', pl: '5', listStyle: 'decimal' })}>
          {traces.map((t, n) => (
            <li key={n}>
              {t.label} → {t.configuration.join(', ') || 'ended'}
              {t.error ? <span className={bad}> ({t.error})</span> : null}
            </li>
          ))}
        </ol>
        <div className={css({ display: 'flex', gap: '2' })}>
          <input className={input} placeholder="Scenario name" value={name} onChange={(e) => setName(e.target.value)} />
          <button
            type="button"
            className={ghost}
            disabled={name === '' || played.length === 0}
            onClick={() => {
              model.setScenarios([...model.scenarios, { name, guards: scenario.guards, steps: traces.map((t, n) => ({ ...(n === 0 ? {} : played[n - 1]), expect: t.configuration })) }])
              setName('')
            }}
          >
            Save as scenario
          </button>
        </div>
        <span className={muted}>
          Saved into the package's tests/ folder; {model.scenarios.length} scenario{model.scenarios.length === 1 ? '' : 's'} now.
        </span>
      </fieldset>
    </div>
  )
}

function VariantB({ model }: { model: Model }) {
  const [selected, setSelected] = useState<string | null>(null)
  const [mode, setMode] = useState<'edit' | 'play'>('edit')
  const [trace, setTrace] = useState<Trace | undefined>(undefined)
  const found = useMemo(() => checks(model.def, model.scenarios), [model.def, model.scenarios])
  const playing = mode === 'play' ? trace : undefined
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <ChecksPanel found={found} onSelect={(id) => (setSelected(id), setMode('edit'))} onGrant={model.grant} />
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '16rem 1fr 22rem' }, gap: '3' })}>
        <nav aria-label="Outline" className={cx(box, css({ p: '2', maxHeight: '640px', overflowY: 'auto' }))}>
          <Outline model={model} selected={selected} onSelect={(id) => (setSelected(id), setMode('edit'))} active={playing?.configuration ?? []} />
        </nav>
        <div className={box}>
          <WorkflowCanvas def={model.def} selected={selected} onSelect={setSelected} active={playing?.configuration} taken={playing?.taken} flagged={playing ? [] : flaggedIds(found)} onConnect={(f, t) => setSelected(model.addTransition(f, t))} height={640} />
        </div>
        <aside className={cx(box, css({ p: '3', maxHeight: '640px', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '3' }))}>
          <div role="tablist" className={css({ display: 'flex', gap: '1' })}>
            {(['edit', 'play'] as const).map((m) => (
              <button key={m} role="tab" type="button" aria-selected={mode === m} className={cx(ghost, mode === m ? css({ bg: 'bg.selected', borderColor: 'accent.default' }) : undefined)} onClick={() => setMode(m)}>
                {m === 'edit' ? 'Inspect' : 'Sandbox'}
              </button>
            ))}
          </div>
          {mode === 'edit' ? <Inspector model={model} selected={selected} onSelect={setSelected} /> : <Sandbox model={model} onTrace={setTrace} />}
        </aside>
      </div>
    </div>
  )
}

// --- Variant C: tables, a map, and scenarios as expected-against-actual ------------------------

function VariantC({ model }: { model: Model }) {
  const { def } = model
  const [tab, setTab] = useState<'states' | 'transitions' | 'scenarios'>('transitions')
  const [selected, setSelected] = useState<string | null>(null)
  const [highlight, setHighlight] = useState<Trace | undefined>(undefined)
  const found = useMemo(() => checks(def, model.scenarios), [def, model.scenarios])
  const flagged = flaggedIds(found)
  const cell = css({ px: '2', py: '1', borderBottom: '1px solid', borderColor: 'border', verticalAlign: 'top', fontSize: 'sm' })
  const row = (id: string) => cx(css({ cursor: 'pointer' }), selected === id ? css({ bg: 'bg.selected' }) : flagged.includes(id) ? css({ bg: 'bg.subtle' }) : undefined)
  const plain = cx(input, css({ minHeight: '1.8rem', fontSize: 'sm' }))
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <ChecksPanel found={found} onSelect={(id) => (setSelected(id), setTab(def.states.some((x) => x.id === id) ? 'states' : 'transitions'))} onGrant={model.grant} />
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 26rem' }, gap: '3', alignItems: 'start' })}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', minWidth: 0 })}>
          <div role="tablist" className={css({ display: 'flex', gap: '1' })}>
            {(['transitions', 'states', 'scenarios'] as const).map((t) => (
              <button key={t} role="tab" type="button" aria-selected={tab === t} className={cx(ghost, tab === t ? css({ bg: 'bg.selected', borderColor: 'accent.default' }) : undefined)} onClick={() => setTab(t)}>
                {t === 'transitions' ? `Transitions (${def.transitions.length})` : t === 'states' ? `States (${def.states.length})` : `Scenarios (${model.scenarios.length})`}
              </button>
            ))}
          </div>
          {tab === 'transitions' ? (
            <div className={cx(box, css({ overflowX: 'auto' }))}>
              <table className={css({ width: '100%', borderCollapse: 'collapse' })}>
                <thead>
                  <tr>
                    {['From', 'On', 'Only if', 'Do', 'To'].map((h) => (
                      <th key={h} className={cx(cell, css({ textAlign: 'left' }))}>
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {def.transitions.map((tr) => (
                    <tr key={tr.id} className={row(tr.id)} onClick={() => setSelected(tr.id)}>
                      <td className={cell}>
                        <select className={plain} value={tr.from} onChange={(e) => model.editTransition(tr.id, { from: e.target.value })}>
                          {def.states.map((x) => (
                            <option key={x.id} value={x.id}>
                              {x.label}
                            </option>
                          ))}
                        </select>
                      </td>
                      <td className={cell}>
                        <input className={cx(plain, css({ fontFamily: 'mono' }))} value={tr.event} placeholder="(eventless)" onChange={(e) => model.editTransition(tr.id, { event: e.target.value })} />
                      </td>
                      <td className={cx(cell, css({ fontFamily: 'mono', fontSize: 'xs' }))}>{guardText(tr.guard) || '—'}</td>
                      <td className={cx(cell, css({ fontSize: 'xs' }))}>{tr.actions.map(describeAction).join('; ') || '—'}</td>
                      <td className={cell}>
                        <select className={plain} value={tr.to} onChange={(e) => model.editTransition(tr.id, { to: e.target.value })}>
                          {def.states.map((x) => (
                            <option key={x.id} value={x.id}>
                              {x.label}
                            </option>
                          ))}
                        </select>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <button type="button" className={cx(ghost, css({ m: '2' }))} onClick={() => setSelected(model.addTransition(def.initial, def.initial))}>
                Add transition
              </button>
            </div>
          ) : null}
          {tab === 'states' ? (
            <div className={cx(box, css({ overflowX: 'auto' }))}>
              <table className={css({ width: '100%', borderCollapse: 'collapse' })}>
                <thead>
                  <tr>
                    {['Name', 'Kind', 'Inside', 'What happens', 'On entry'].map((h) => (
                      <th key={h} className={cx(cell, css({ textAlign: 'left' }))}>
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {def.states.map((st) => (
                    <tr key={st.id} className={row(st.id)} onClick={() => setSelected(st.id)}>
                      <td className={cell}>
                        <input className={plain} value={st.label} onChange={(e) => model.editState(st.id, { label: e.target.value })} />
                      </td>
                      <td className={cell}>
                        <select className={plain} value={st.kind} onChange={(e) => model.editState(st.id, { kind: e.target.value as Kind })}>
                          {['atomic', 'compound', 'parallel', 'final'].map((k) => (
                            <option key={k}>{k}</option>
                          ))}
                        </select>
                      </td>
                      <td className={cell}>{st.parent ? stateLabel(def, st.parent) : '—'}</td>
                      <td className={cx(cell, css({ fontSize: 'xs' }))}>{st.task ? `${st.task.role} task on ${st.task.worklist}${st.task.form ? `, form ${st.task.form.split('/').pop()}` : ''}` : st.gate ? `gate ${st.gate}` : st.invoke ? `runs ${st.invoke.split('/').pop()}` : '—'}</td>
                      <td className={cx(cell, css({ fontSize: 'xs' }))}>{st.entry.map(describeAction).join('; ') || '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <button type="button" className={cx(ghost, css({ m: '2' }))} onClick={() => setSelected(model.addState(null))}>
                Add state
              </button>
            </div>
          ) : null}
          {tab === 'scenarios' ? (
            <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
              {model.scenarios.map((sc) => {
                const traces = simulate(def, sc)
                return (
                  <div key={sc.name} className={cx(box, css({ overflowX: 'auto' }))}>
                    <div className={css({ p: '2', fontWeight: 'semibold' })}>
                      {traces.every((t) => t.pass) ? <span className={ok}>✓</span> : <span className={bad}>✗</span>} {sc.name}{' '}
                      <span className={muted}>
                        guards: {Object.entries(sc.guards).map(([k, v]) => `${k} → ${v}`).join('; ') || 'none'}
                      </span>
                    </div>
                    <table className={css({ width: '100%', borderCollapse: 'collapse' })}>
                      <thead>
                        <tr>
                          {['#', 'Send', 'Expected', 'Actual', 'Actions (stubbed)'].map((h) => (
                            <th key={h} className={cx(cell, css({ textAlign: 'left' }))}>
                              {h}
                            </th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {traces.map((t, n) => (
                          <tr key={n} className={cx(css({ cursor: 'pointer' }), highlight === t ? css({ bg: 'bg.selected' }) : undefined)} onClick={() => setHighlight(t)}>
                            <td className={cell}>{n + 1}</td>
                            <td className={cx(cell, css({ fontFamily: 'mono', fontSize: 'xs' }))}>{t.label}</td>
                            <td className={cx(cell, css({ fontSize: 'xs' }))}>{t.expected.join(', ')}</td>
                            <td className={cx(cell, css({ fontSize: 'xs' }), t.pass ? ok : bad)}>{t.error ?? t.configuration.join(', ')}</td>
                            <td className={cx(cell, css({ fontSize: 'xs' }))}>{t.actions.join('; ') || '—'}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )
              })}
            </div>
          ) : null}
        </div>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3', position: { lg: 'sticky' }, top: '4' })}>
          <div className={box}>
            <WorkflowCanvas def={def} selected={selected} onSelect={setSelected} active={tab === 'scenarios' ? highlight?.configuration : undefined} taken={tab === 'scenarios' ? highlight?.taken : undefined} flagged={flagged} height={380} compact />
          </div>
          {tab !== 'scenarios' && selected ? (
            <details open className={cx(box, css({ p: '3', maxHeight: '420px', overflowY: 'auto' }))}>
              <summary className={css({ cursor: 'pointer', fontWeight: 'semibold' })}>Details of the selected row</summary>
              <Inspector model={model} selected={selected} onSelect={setSelected} />
            </details>
          ) : null}
        </div>
      </div>
    </div>
  )
}

// --- The shell -----------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'Canvas and inspector; scenarios replayed on the canvas', View: VariantA },
  { key: 'B', name: 'Outline, canvas and a free-play sandbox', View: VariantB },
  { key: 'C', name: 'Tables with a map; scenarios as expected vs actual', View: VariantC },
] as const

export function WorkflowStudioPrototype() {
  const model = useDefinition()
  const [variant, setVariant] = useState(() => new URLSearchParams(window.location.search).get('variant') ?? 'A')
  const index = Math.max(
    0,
    VARIANTS.findIndex((v) => v.key === variant),
  )
  const current = VARIANTS[index] ?? VARIANTS[0]
  const go = (step: number) => {
    const next = VARIANTS[(index + step + VARIANTS.length) % VARIANTS.length] ?? VARIANTS[0]
    const url = new URL(window.location.href)
    url.searchParams.set('variant', next.key)
    window.history.replaceState(null, '', url)
    setVariant(next.key)
  }
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest('input, textarea, select, [contenteditable], .react-flow')) return
      if (e.key === 'ArrowLeft') go(-1)
      if (e.key === 'ArrowRight') go(1)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
  const View = current.View
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', pb: '16' })}>
      <header className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', flexWrap: 'wrap', gap: '3' })}>
        <div>
          <p className={muted}>← OPD (ug.moh.opd 2.1.0, draft) · Workflow Definition</p>
          <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{model.def.title}</h1>
        </div>
        <span className={muted}>Saved · revision {model.revision} · you hold this draft</span>
      </header>
      <View key={current.key} model={model} />
      {import.meta.env.PROD ? null : (
        <div role="toolbar" aria-label="Prototype variants" className={css({ position: 'fixed', bottom: '4', left: '50%', transform: 'translateX(-50%)', display: 'flex', gap: '3', alignItems: 'center', bg: 'fg.default', color: 'bg.canvas', px: '4', py: '2', borderRadius: 'full', boxShadow: 'lg', zIndex: '10' })}>
          <button type="button" aria-label="Previous variant" onClick={() => go(-1)} className={css({ cursor: 'pointer' })}>
            ←
          </button>
          <span>
            {current.key} ({current.name})
          </span>
          <button type="button" aria-label="Next variant" onClick={() => go(1)} className={css({ cursor: 'pointer' })}>
            →
          </button>
        </div>
      )}
    </section>
  )
}
