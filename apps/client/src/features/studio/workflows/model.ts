// The Workflow Studio's model (studio.md § The Workflow Studio; workflow-engine.md; #498), as
// pure functions. It reads a Workflow Definition from its Library's inline
// `application/vnd.health.workflow+json` content (ADR 0049), edits its statechart in place and
// writes it back, keeping whatever it does not edit (exit actions, invocations' ids, decision
// tables, the data model) as read. The shapes are the Rust engine's (`health_workflow::model`
// and `::definition`), field for field.

/** A data-model value: typed fields only (ADR 0042). */
export type Value = boolean | number | string

export const KINDS = ['atomic', 'compound', 'parallel', 'final'] as const
export type Kind = (typeof KINDS)[number]

export type Action = { kind: 'raise'; event: string } | { kind: 'assign'; field: string; value: Value } | { kind: 'execute'; action: string; params?: Record<string, Value> }

export interface Transition {
  /** Event descriptors; none: eventless, taken as soon as its guard holds. */
  events?: string[]
  /** A FHIRPath guard. */
  guard?: string | null
  targets?: string[]
  internal?: boolean
  actions?: Action[]
}

/** A durable timer: `event` raised `after` (ISO 8601) the state is entered. */
export interface Timer {
  after: string
  event: string
}

export interface Invoke {
  id: string
  src: string
}

export const PRIORITIES = ['routine', 'urgent', 'asap', 'stat'] as const
export type Priority = (typeof PRIORITIES)[number]

export interface HumanTask {
  role: string
  form?: string | null
  worklist: string
  priority?: Priority
  claimTimeout?: string
}

export interface State {
  id: string
  kind: Kind
  initial?: string[]
  children?: State[]
  on_entry?: Action[][]
  on_exit?: Action[][]
  transitions?: Transition[]
  timers?: Timer[]
  invoke?: Invoke[]
  task?: HumanTask | null
  gate?: string | null
}

export interface Statechart {
  states: State[]
  initial?: string[]
}

export interface Field {
  name: string
  kind: 'bool' | 'int' | 'text'
}

export interface DataRequirement {
  name: string
  resourceType: string
  paths?: string[]
}

export interface DecisionTable {
  id: string
  rows: { when: string[]; then: Value }[]
  default?: Value | null
}

export interface WorkflowDefinition {
  url: string
  version: string
  statechart: Statechart
  dataModel?: Field[]
  dataRequirements?: DataRequirement[]
  decisionTables?: DecisionTable[]
  principal?: string[]
}

// ---------------------------------------------------------------------------------------------
// The Library

export const WORKFLOW_CONTENT_TYPE = 'application/vnd.health.workflow+json'

/** A Library as JSON. */
export type Library = Record<string, unknown> & { resourceType: 'Library'; content?: Record<string, unknown>[] }

/** Whether `value` is a Workflow Definition Library: its type is the platform's `workflow-definition`. */
export function isWorkflowLibrary(value: unknown): value is Library {
  if (typeof value !== 'object' || value === null) return false
  const library = value as { resourceType?: unknown; type?: { coding?: { code?: unknown }[] } }
  return library.resourceType === 'Library' && (library.type?.coding ?? []).some((c) => c.code === 'workflow-definition')
}

const decode = (data: string) => new TextDecoder().decode(Uint8Array.from(atob(data), (c) => c.codePointAt(0) ?? 0))
const encode = (text: string) => btoa(Array.from(new TextEncoder().encode(text), (b) => String.fromCodePoint(b)).join(''))

export type ReadRefusal = 'not-json' | 'not-workflow' | 'not-inline' | 'unreadable'

/** The definition a Library's text holds, or why it holds none the Studio can open. */
export function readDefinition(text: string): { library: Library; definition: WorkflowDefinition } | { refusal: ReadRefusal } {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return { refusal: 'not-json' }
  }
  if (!isWorkflowLibrary(value)) return { refusal: 'not-workflow' }
  const content = value.content?.[0]
  if (content?.['contentType'] !== WORKFLOW_CONTENT_TYPE || typeof content['data'] !== 'string') return { refusal: 'not-inline' }
  try {
    const definition = JSON.parse(decode(content['data'])) as WorkflowDefinition
    if (typeof definition !== 'object' || definition === null || !Array.isArray(definition.statechart?.states)) return { refusal: 'unreadable' }
    return { library: value, definition }
  } catch {
    return { refusal: 'unreadable' }
  }
}

/** `library` holding `definition` as its inline content, everything else as read. */
export function writeDefinition(library: Library, definition: WorkflowDefinition): Library {
  const [first, ...rest] = library.content ?? []
  return { ...library, content: [{ ...first, contentType: WORKFLOW_CONTENT_TYPE, data: encode(JSON.stringify(definition, null, 2)) }, ...rest] }
}

// ---------------------------------------------------------------------------------------------
// Reading the statechart

/** One state, where it sits and how deep. */
export interface Row {
  state: State
  parent: string | null
  depth: number
}

/** Every state, depth first, in document order. */
export function rows(definition: WorkflowDefinition): Row[] {
  const out: Row[] = []
  const walk = (states: State[], parent: string | null, depth: number) => {
    for (const state of states) {
      out.push({ state, parent, depth })
      walk(state.children ?? [], state.id, depth + 1)
    }
  }
  walk(definition.statechart.states, null, 0)
  return out
}

export function stateOf(definition: WorkflowDefinition, id: string): State | undefined {
  return rows(definition).find((r) => r.state.id === id)?.state
}

export function parentOf(definition: WorkflowDefinition, id: string): string | null {
  return rows(definition).find((r) => r.state.id === id)?.parent ?? null
}

/** `id` and every state inside it. */
export function descendants(definition: WorkflowDefinition, id: string): Set<string> {
  const out = new Set<string>()
  const walk = (state: State) => {
    out.add(state.id)
    for (const child of state.children ?? []) walk(child)
  }
  const state = stateOf(definition, id)
  if (state !== undefined) walk(state)
  return out
}

/** What is selected: a state, or a transition by its source and its index there. */
export type Selection = { kind: 'state'; id: string } | { kind: 'transition'; source: string; index: number }

/** The canvas's id of a transition. */
export const edgeId = (source: string, index: number) => `${source}→${index}`

/** One transition, flattened. */
export interface Edge {
  source: string
  index: number
  transition: Transition
}

export function edges(definition: WorkflowDefinition): Edge[] {
  return rows(definition).flatMap(({ state }) => (state.transitions ?? []).map((transition, index) => ({ source: state.id, index, transition })))
}

export function transitionAt(definition: WorkflowDefinition, source: string, index: number): Transition | undefined {
  return stateOf(definition, source)?.transitions?.[index]
}

/** What triggers a transition: an event, a timer of its source state, or nothing. */
export type Trigger = { kind: 'event'; events: string[] } | { kind: 'timer'; after: string; event: string } | { kind: 'eventless' }

export function triggerOf(state: State, transition: Transition): Trigger {
  const events = transition.events ?? []
  if (events.length === 0) return { kind: 'eventless' }
  const [only] = events
  const timer = events.length === 1 ? (state.timers ?? []).find((t) => t.event === only) : undefined
  return timer === undefined ? { kind: 'event', events } : { kind: 'timer', after: timer.after, event: timer.event }
}

/** Whether an event descriptor matches an event (`foo` matches `foo` and `foo.bar`; `*` any). */
const matches = (descriptor: string, event: string) => descriptor === '*' || descriptor === event || event.startsWith(`${descriptor}.`)

/** The other transitions of the same state that the same event could take, in document order:
 * the first whose guard holds wins. */
export function competitors(state: State, index: number): { index: number; transition: Transition }[] {
  const all = state.transitions ?? []
  const own = all[index]?.events ?? []
  return all
    .map((transition, n) => ({ index: n, transition }))
    .filter(({ index: n, transition }) => {
      if (n === index) return false
      const theirs = transition.events ?? []
      if (own.length === 0 || theirs.length === 0) return own.length === 0 && theirs.length === 0
      return own.some((a) => theirs.some((b) => matches(a, b) || matches(b, a)))
    })
}

// ---------------------------------------------------------------------------------------------
// Editing

/** `T` with any field possibly undefined, before `tidy` leaves those out. */
type Loose<T> = { [K in keyof T]?: T[K] | undefined }

/** Leaves out the optional fields that are empty, as the engine's defaults read them. */
function tidy<T extends object>(value: Loose<T>): T {
  const out: Record<string, unknown> = {}
  for (const [key, field] of Object.entries(value)) {
    if (field === undefined || field === null || (Array.isArray(field) && field.length === 0 && key !== 'states')) continue
    out[key] = field
  }
  return out as T
}

function mapStates(states: State[], f: (state: State) => State): State[] {
  return states.map((state) => f(state.children === undefined ? state : { ...state, children: mapStates(state.children, f) }))
}

function withStates(definition: WorkflowDefinition, f: (states: State[]) => State[]): WorkflowDefinition {
  return { ...definition, statechart: { ...definition.statechart, states: f(definition.statechart.states) } }
}

export function updateState(definition: WorkflowDefinition, id: string, f: (state: State) => State): WorkflowDefinition {
  return withStates(definition, (states) => mapStates(states, (state) => (state.id === id ? tidy(f(state)) : state)))
}

/** A state id not yet used. */
export function freshId(definition: WorkflowDefinition, base: string): string {
  const used = new Set(rows(definition).map((r) => r.state.id))
  if (!used.has(base)) return base
  let n = 2
  while (used.has(`${base}-${n}`)) n += 1
  return `${base}-${n}`
}

/** A new atomic state inside `parent` (or at the top level), last in document order. */
export function addState(definition: WorkflowDefinition, parent: string | null): { definition: WorkflowDefinition; id: string } {
  const id = freshId(definition, 'state')
  const added: State = { id, kind: 'atomic' }
  if (parent === null) return { definition: withStates(definition, (states) => [...states, added]), id }
  return { definition: updateState(definition, parent, (state) => ({ ...state, children: [...(state.children ?? []), added] })), id }
}

const renamed = (ids: string[] | undefined, from: string, to: string) => ids?.map((x) => (x === from ? to : x))

/** `id` renamed `to`, and every reference to it: targets, initial states, its done event. */
export function renameState(definition: WorkflowDefinition, id: string, to: string): WorkflowDefinition {
  const done = (event: string) => (event === `done.state.${id}` ? `done.state.${to}` : event)
  const states = mapStates(definition.statechart.states, (state) =>
    tidy({
      ...state,
      id: state.id === id ? to : state.id,
      initial: renamed(state.initial, id, to),
      transitions: state.transitions?.map((t) => tidy({ ...t, targets: renamed(t.targets, id, to), events: t.events?.map(done) })),
    }),
  )
  return { ...definition, statechart: tidy({ ...definition.statechart, states, initial: renamed(definition.statechart.initial, id, to) }) }
}

function without(states: State[], gone: Set<string>): State[] {
  return states
    .filter((state) => !gone.has(state.id))
    .map((state) =>
      tidy({
        ...state,
        initial: state.initial?.filter((x) => !gone.has(x)),
        children: state.children === undefined ? undefined : without(state.children, gone),
        // A transition that only went to a removed state goes too.
        transitions: state.transitions
          ?.filter((t) => (t.targets ?? []).length === 0 || (t.targets ?? []).some((x) => !gone.has(x)))
          .map((t) => tidy({ ...t, targets: t.targets?.filter((x) => !gone.has(x)) })),
      }),
    )
}

/** The definition without `id`, the states inside it, and the transitions that went only there. */
export function removeState(definition: WorkflowDefinition, id: string): WorkflowDefinition {
  const gone = descendants(definition, id)
  return { ...definition, statechart: tidy({ ...definition.statechart, states: without(definition.statechart.states, gone), initial: definition.statechart.initial?.filter((x) => !gone.has(x)) }) }
}

/** `id` moved inside `parent` (or to the top level), last there. Not into itself. */
export function moveState(definition: WorkflowDefinition, id: string, parent: string | null): WorkflowDefinition {
  const state = stateOf(definition, id)
  if (state === undefined || (parent !== null && descendants(definition, id).has(parent)) || parentOf(definition, id) === parent) return definition
  const pulled = (states: State[]): State[] =>
    states.filter((s) => s.id !== id).map((s) => (s.children === undefined ? s : tidy({ ...s, children: pulled(s.children), initial: s.initial?.filter((x) => x !== id) })))
  const out = withStates(definition, pulled)
  if (parent === null) return withStates(out, (states) => [...states, state])
  return updateState(out, parent, (p) => ({ ...p, children: [...(p.children ?? []), state] }))
}

/** `state` as another kind: only an atomic state has a task or a gate. */
export function withKind(state: State, kind: Kind): State {
  if (kind === 'atomic') return { ...state, kind }
  const { task: _task, gate: _gate, ...rest } = state
  return { ...rest, kind }
}

/** What happens while a state is active. */
export type Activity = 'wait' | 'task' | 'gate' | 'invoke'

export function activityOf(state: State): Activity {
  if (state.gate !== undefined && state.gate !== null) return 'gate'
  if (state.task !== undefined && state.task !== null) return 'task'
  if ((state.invoke ?? []).length > 0) return 'invoke'
  return 'wait'
}

/** `state` doing `activity` instead, with blank fields to fill. A gate's task is the cashier's. */
export function withActivity(state: State, activity: Activity): State {
  const { task, gate: _gate, invoke: _invoke, ...rest } = state
  switch (activity) {
    case 'wait':
      return rest
    case 'task':
      return { ...rest, task: task ?? { role: '', worklist: '' } }
    case 'gate':
      return { ...rest, gate: '', task: task ?? { role: 'cashier', worklist: 'cashier' } }
    case 'invoke':
      return { ...rest, invoke: [{ id: state.id, src: '' }] }
  }
}

export function updateTransition(definition: WorkflowDefinition, source: string, index: number, f: (t: Transition) => Transition): WorkflowDefinition {
  return updateState(definition, source, (state) => ({ ...state, transitions: (state.transitions ?? []).map((t, n) => (n === index ? tidy(f(t)) : t)) }))
}

/** A new transition from `source` to `target` on an event named for them, last in document order. */
export function addTransition(definition: WorkflowDefinition, source: string, target: string): { definition: WorkflowDefinition; index: number } {
  const index = (stateOf(definition, source)?.transitions ?? []).length
  const added: Transition = { events: [`${source}.to-${target}`], targets: [target] }
  return { definition: updateState(definition, source, (state) => ({ ...state, transitions: [...(state.transitions ?? []), added] })), index }
}

/** Drops timers no transition of the state is taken on any more. */
function pruneTimers(state: State): State {
  const used = new Set((state.transitions ?? []).flatMap((t) => t.events ?? []))
  return tidy({ ...state, timers: state.timers?.filter((t) => used.has(t.event)) })
}

export function removeTransition(definition: WorkflowDefinition, source: string, index: number): WorkflowDefinition {
  return updateState(definition, source, (state) => pruneTimers({ ...state, transitions: (state.transitions ?? []).filter((_, n) => n !== index) }))
}

/** The transition one place earlier (-1) or later (+1) in document order, and its new index. */
export function moveTransition(definition: WorkflowDefinition, source: string, index: number, by: -1 | 1): { definition: WorkflowDefinition; index: number } {
  const list = [...(stateOf(definition, source)?.transitions ?? [])]
  const to = index + by
  const moving = list[index]
  if (moving === undefined || to < 0 || to >= list.length) return { definition, index }
  list.splice(index, 1)
  list.splice(to, 0, moving)
  return { definition: updateState(definition, source, (state) => ({ ...state, transitions: list })), index: to }
}

/** The transition moved to leave `to` instead, last there; a timer it was taken on goes along. */
export function moveSource(definition: WorkflowDefinition, source: string, index: number, to: string): { definition: WorkflowDefinition; index: number } {
  const from = stateOf(definition, source)
  const transition = from?.transitions?.[index]
  if (from === undefined || transition === undefined || source === to || stateOf(definition, to) === undefined) return { definition, index }
  const trigger = triggerOf(from, transition)
  const out = removeTransition(definition, source, index)
  const at = (stateOf(out, to)?.transitions ?? []).length
  return {
    definition: updateState(out, to, (state) => ({
      ...state,
      transitions: [...(state.transitions ?? []), transition],
      timers: trigger.kind === 'timer' && !(state.timers ?? []).some((t) => t.event === trigger.event) ? [...(state.timers ?? []), { after: trigger.after, event: trigger.event }] : (state.timers ?? []),
    })),
    index: at,
  }
}

/** The transition triggered otherwise. A timer is the source state's, raising an event of its own. */
export function withTrigger(definition: WorkflowDefinition, source: string, index: number, trigger: Trigger): WorkflowDefinition {
  return updateState(definition, source, (state) => {
    const transitions = state.transitions ?? []
    const current = transitions[index]
    if (current === undefined) return state
    const before = triggerOf(state, current)
    let timers = state.timers ?? []
    let events: string[]
    switch (trigger.kind) {
      case 'eventless':
        events = []
        break
      case 'event':
        events = trigger.events
        break
      case 'timer': {
        const event = before.kind === 'timer' ? before.event : freshEvent(state, `${state.id}.timeout`)
        timers = before.kind === 'timer' ? timers.map((t) => (t.event === event ? { ...t, after: trigger.after } : t)) : [...timers, { after: trigger.after, event }]
        events = [event]
        break
      }
    }
    return pruneTimers({ ...state, timers, transitions: transitions.map((t, n) => (n === index ? tidy({ ...t, events }) : t)) })
  })
}

function freshEvent(state: State, base: string): string {
  const used = new Set([...(state.timers ?? []).map((t) => t.event), ...(state.transitions ?? []).flatMap((t) => t.events ?? [])])
  if (!used.has(base)) return base
  let n = 2
  while (used.has(`${base}-${n}`)) n += 1
  return `${base}-${n}`
}

/** A state's entry actions: its first entry block (the others, if any, are kept as read). */
export const entryActions = (state: State): Action[] => state.on_entry?.[0] ?? []

export function withEntryActions(state: State, actions: Action[]): State {
  const [, ...rest] = state.on_entry ?? []
  return tidy({ ...state, on_entry: actions.length === 0 && rest.length === 0 ? undefined : [actions, ...rest] })
}

// ---------------------------------------------------------------------------------------------
// Publication checks

/** What the Studio finds while editing (workflow-engine.md § Simulation and publication gates).
 * The server's `WorkflowDefinition::problems` at *Check now* remains the authority; these are
 * the checks the browser can run without the release's action catalogue and the simulator. */
export type Problem =
  | { code: 'duplicate-id' | 'bad-children' | 'bad-initial' | 'unreachable' | 'dead-end' | 'bad-task' | 'bad-gate' | 'no-id'; state: string }
  | { code: 'unknown-target'; source: string; index: number; target: string }
  | { code: 'bad-assignment'; at: Selection; field: string }
  | { code: 'bad-table'; table: string }
  | { code: 'unknown-initial'; target: string }

/** The element a problem is about, if it is one. */
export function problemAt(problem: Problem): Selection | null {
  if ('state' in problem) return { kind: 'state', id: problem.state }
  if (problem.code === 'unknown-target') return { kind: 'transition', source: problem.source, index: problem.index }
  if (problem.code === 'bad-assignment') return problem.at
  return null
}

const kindOf = (value: Value): Field['kind'] => (typeof value === 'boolean' ? 'bool' : typeof value === 'number' ? 'int' : 'text')
const isVersioned = (canonical: string) => {
  const [url, version] = canonical.split('|')
  return canonical.split('|').length === 2 && url !== '' && version !== ''
}

export function problems(definition: WorkflowDefinition): Problem[] {
  const out: Problem[] = []
  const all = rows(definition)
  const ids = new Set<string>()
  for (const { state } of all) {
    if (state.id.trim() === '') out.push({ code: 'no-id', state: state.id })
    else if (ids.has(state.id)) out.push({ code: 'duplicate-id', state: state.id })
    ids.add(state.id)
  }
  for (const { state } of all) {
    const container = state.kind === 'compound' || state.kind === 'parallel'
    if (container === ((state.children ?? []).length === 0)) out.push({ code: 'bad-children', state: state.id })
    if (state.kind === 'compound' && (state.initial ?? []).some((x) => !descendants(definition, state.id).has(x) || x === state.id)) out.push({ code: 'bad-initial', state: state.id })
    ;(state.transitions ?? []).forEach((t, index) => {
      for (const target of t.targets ?? []) if (!ids.has(target)) out.push({ code: 'unknown-target', source: state.id, index, target })
    })
  }
  for (const target of definition.statechart.initial ?? []) if (!ids.has(target)) out.push({ code: 'unknown-initial', target })
  // As in the engine, a statechart that does not compile is not checked further.
  if (out.length > 0) return out

  const fields = new Map((definition.dataModel ?? []).map((f) => [f.name, f.kind]))
  const assignments = (actions: Action[], at: Selection) => {
    for (const action of actions) if (action.kind === 'assign' && fields.get(action.field) !== kindOf(action.value)) out.push({ code: 'bad-assignment', at, field: action.field })
  }
  for (const { state } of all) {
    assignments([...(state.on_entry ?? []), ...(state.on_exit ?? [])].flat(), { kind: 'state', id: state.id })
    ;(state.transitions ?? []).forEach((t, index) => assignments(t.actions ?? [], { kind: 'transition', source: state.id, index }))
  }

  out.push(...reachability(definition, all))

  for (const { state } of all) {
    if (state.gate !== undefined && state.gate !== null) {
      const movesOn = (event: string) => (state.transitions ?? []).some((t) => (t.targets ?? []).length > 0 && (t.events ?? []).includes(event))
      const loops = (state.transitions ?? []).some((t) => (t.events ?? []).includes('gate.block') && (t.targets ?? []).includes(state.id))
      if (state.kind !== 'atomic' || state.gate.trim() === '' || state.task === undefined || state.task === null || !movesOn('gate.pass') || !movesOn('gate.defer') || loops) out.push({ code: 'bad-gate', state: state.id })
    }
    const task = state.task
    if (task !== undefined && task !== null) {
      const form = task.form ?? null
      if (state.kind !== 'atomic' || task.role.trim() === '' || task.worklist.trim() === '' || (form !== null && !isVersioned(form))) out.push({ code: 'bad-task', state: state.id })
    }
  }

  const tables = new Set<string>()
  for (const table of definition.decisionTables ?? []) {
    if (tables.has(table.id) || table.rows.length === 0 || table.rows.some((r) => r.when.length === 0 || r.when.some((c) => c.trim() === ''))) out.push({ code: 'bad-table', table: table.id })
    tables.add(table.id)
  }
  return out
}

/** States nothing can reach, and non-final states with no way out, as the engine finds them. */
function reachability(definition: WorkflowDefinition, all: Row[]): Problem[] {
  const by = new Map(all.map((r) => [r.state.id, r]))
  const ancestors = (id: string): State[] => {
    const out: State[] = []
    let at = by.get(id)?.parent ?? null
    while (at !== null) {
      const row = by.get(at)
      if (row === undefined) break
      out.push(row.state)
      at = row.parent
    }
    return out
  }
  const start = definition.statechart.initial ?? []
  const pending = start.length > 0 ? [...start] : definition.statechart.states.slice(0, 1).map((s) => s.id)
  const reached = new Set<string>()
  while (pending.length > 0) {
    const id = pending.pop() as string
    const state = by.get(id)?.state
    if (state === undefined || reached.has(id)) continue
    reached.add(id)
    pending.push(...ancestors(id).map((s) => s.id))
    const children = state.children ?? []
    if (state.kind === 'parallel') pending.push(...children.map((c) => c.id))
    else if (state.kind === 'compound') pending.push(...((state.initial ?? []).length > 0 ? (state.initial ?? []) : children.slice(0, 1).map((c) => c.id)))
    for (const t of state.transitions ?? []) pending.push(...(t.targets ?? []))
  }
  const out: Problem[] = []
  for (const { state } of all) {
    if (!reached.has(state.id)) {
      out.push({ code: 'unreachable', state: state.id })
      continue
    }
    const leaf = (state.children ?? []).length === 0 && state.kind !== 'final'
    const exits = (state.transitions ?? []).length > 0 || ancestors(state.id).some((a) => (a.transitions ?? []).length > 0)
    if (leaf && !exits) out.push({ code: 'dead-end', state: state.id })
  }
  return out
}

/** Every element a problem is about, for outlining them on the canvas and in the outline. */
export function flagged(found: Problem[]): Set<string> {
  const out = new Set<string>()
  for (const problem of found) {
    const at = problemAt(problem)
    if (at !== null) out.add(at.kind === 'state' ? at.id : edgeId(at.source, at.index))
  }
  return out
}

/** The state ids of the previous published version: they are fixed (running instances and
 * migration maps refer to them). */
export function publishedIds(definition: WorkflowDefinition | null): Set<string> {
  return new Set(definition === null ? [] : rows(definition).map((r) => r.state.id))
}
