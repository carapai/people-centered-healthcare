// PROTOTYPE (#481), throwaway: an in-memory Workflow Definition (the OPD shape of
// workflow-engine.md), a tiny stand-in for the Rust engine's sandbox, and the publication checks.
// The real simulation runs in `health_workflow::simulation` on the server; this one only behaves
// enough like it (SCXML-style, document order, stubbed guards, stubbed actions) to judge the UI.

export type Kind = 'atomic' | 'compound' | 'parallel' | 'final'

export type Action = { id: string; params: Record<string, string> }

export type HumanTask = { role: string; form: string; worklist: string; priority: 'routine' | 'urgent' | 'emergency'; claimTimeout: string }

export type State = {
  id: string
  label: string
  kind: Kind
  parent: string | null
  /** The child entered first, for a compound state. */
  initial?: string | undefined
  task?: HumanTask | undefined
  /** A Financial Gate configuration: exits on gate.pass / gate.defer / gate.block. */
  gate?: string | undefined
  /** A sub-workflow `url|version`; it sends `done.invoke.<state>` back when it finishes. */
  invoke?: string | undefined
  entry: Action[]
}

export type Guard = { kind: 'none' } | { kind: 'fhirpath'; expression: string } | { kind: 'table'; table: string; outcome: string }

export type Transition = {
  id: string
  from: string
  to: string
  /** An event name, `after <ISO duration>` for a timer, or '' for an eventless transition. */
  event: string
  guard: Guard
  actions: Action[]
}

export type DecisionTable = { id: string; title: string; input: string; rows: { when: string; outcome: string }[] }

export type Definition = {
  url: string
  version: string
  title: string
  initial: string
  states: State[]
  transitions: Transition[]
  tables: DecisionTable[]
  /** What the guards may read (the declared context bundle and data model). */
  dataRequirements: string[]
  /** Permissions the workflow principal holds for timer and eventless transitions, approved at publication. */
  principal: string[]
}

export type Param = { name: string; type: 'string' | 'code' | 'duration'; required: boolean; values?: string[] | undefined }

/** The typed actions the contracts register, with their generated parameter schemas (ADR 0043). */
export const ACTIONS: { id: string; title: string; permission: string; compensation: string; params: Param[] }[] = [
  {
    id: 'patient-flow/open-queue-entry',
    title: 'Open a queue entry',
    permission: 'patient-flow.manage',
    compensation: 'patient-flow/close-queue-entry',
    params: [
      { name: 'servicePoint', type: 'code', required: true, values: ['reception', 'triage', 'consultation', 'laboratory', 'pharmacy'] },
      { name: 'priority', type: 'code', required: false, values: ['routine', 'urgent', 'emergency'] },
    ],
  },
  { id: 'patient-flow/close-queue-entry', title: 'Close the queue entry', permission: 'patient-flow.manage', compensation: 'none', params: [] },
  { id: 'worklists/raise-priority', title: 'Raise the task priority', permission: 'worklists.manage', compensation: 'none', params: [{ name: 'to', type: 'code', required: true, values: ['urgent', 'emergency'] }] },
  {
    id: 'communication/notify',
    title: 'Notify a role',
    permission: 'communication.send',
    compensation: 'none',
    params: [
      { name: 'role', type: 'string', required: true },
      { name: 'message', type: 'string', required: true },
    ],
  },
  { id: 'encounter/complete-encounter', title: 'Complete the encounter', permission: 'encounter.manage', compensation: 'none', params: [] },
]

export const ROLES = ['receptionist', 'nurse', 'clinician', 'lab-technician', 'pharmacist', 'cashier']
export const FORMS = ['', 'https://health.go.ug/fhir/Questionnaire/opd-triage|1.0.0', 'https://health.go.ug/fhir/Questionnaire/opd-consultation|1.0.0', 'https://health.go.ug/fhir/Questionnaire/opd-disposition|1.0.0']
export const SUBWORKFLOWS = ['https://health.go.ug/fhir/workflow/lab-order|1.2.0', 'https://health.go.ug/fhir/workflow/pharmacy-dispense|1.0.0', 'https://health.go.ug/fhir/workflow/imaging-order|1.0.0']
export const GATES = ['opd-consultation-fee', 'lab-fee', 'pharmacy-fee']

const task = (role: string, worklist: string, form = ''): HumanTask => ({ role, worklist, form, priority: 'routine', claimTimeout: 'PT15M' })
const s = (id: string, label: string, kind: Kind, parent: string | null, more: Partial<State> = {}): State => ({ id, label, kind, parent, entry: [], ...more })
const t = (id: string, from: string, event: string, to: string, guard: Guard = { kind: 'none' }, actions: Action[] = []): Transition => ({ id, from, event, to, guard, actions })

export const SAMPLE: Definition = {
  url: 'https://health.go.ug/fhir/workflow/opd-visit',
  version: '2.1.0',
  title: 'OPD visit',
  initial: 'reception',
  states: [
    s('reception', 'Reception', 'atomic', null, { task: task('receptionist', 'reception'), entry: [{ id: 'patient-flow/open-queue-entry', params: { servicePoint: 'reception' } }] }),
    s('triage', 'Triage', 'atomic', null, { task: task('nurse', 'triage', FORMS[1]), entry: [{ id: 'patient-flow/open-queue-entry', params: { servicePoint: 'triage' } }] }),
    s('payment', 'Consultation fee', 'atomic', null, { gate: 'opd-consultation-fee' }),
    s('consultation', 'Consultation', 'compound', null, { initial: 'assessing', entry: [{ id: 'patient-flow/open-queue-entry', params: { servicePoint: 'consultation' } }] }),
    s('assessing', 'Assessing', 'atomic', 'consultation', { task: task('clinician', 'consultation', FORMS[2]) }),
    s('orders-placed', 'Orders placed', 'atomic', 'consultation'),
    s('orders', 'Orders', 'parallel', null),
    s('lab', 'Laboratory', 'compound', 'orders', { initial: 'lab-running' }),
    s('lab-running', 'Lab order', 'atomic', 'lab', { invoke: SUBWORKFLOWS[0] }),
    s('lab-done', 'Results back', 'final', 'lab'),
    s('pharmacy', 'Pharmacy', 'compound', 'orders', { initial: 'pharmacy-running' }),
    s('pharmacy-running', 'Dispensing', 'atomic', 'pharmacy', { invoke: SUBWORKFLOWS[1] }),
    s('pharmacy-done', 'Dispensed', 'final', 'pharmacy'),
    s('disposition', 'Disposition', 'atomic', null, { task: task('clinician', 'consultation', FORMS[3]) }),
    s('admission', 'Admission', 'atomic', null, { task: task('nurse', 'ward') }),
    s('done', 'Done', 'final', null),
  ],
  transitions: [
    t('t1', 'reception', 'reception.checked-in', 'triage'),
    t('t2', 'triage', 'triage.completed', 'consultation', { kind: 'table', table: 'triage-route', outcome: 'bypass' }),
    t('t3', 'triage', 'triage.completed', 'payment', { kind: 'fhirpath', expression: "%gate.where(id = 'opd-consultation-fee').active" }),
    t('t4', 'triage', 'triage.completed', 'consultation'),
    t('t5', 'triage', 'after PT30M', 'triage', { kind: 'none' }, [
      { id: 'worklists/raise-priority', params: { to: 'urgent' } },
      { id: 'communication/notify', params: { role: 'nurse-in-charge', message: 'A patient has waited 30 minutes for triage' } },
    ]),
    t('t6', 'payment', 'gate.pass', 'consultation'),
    t('t7', 'payment', 'gate.defer', 'consultation'),
    t('t8', 'assessing', 'orders.placed', 'orders-placed'),
    t('t9', 'orders-placed', '', 'orders'),
    t('t10', 'lab-running', 'done.invoke.lab-running', 'lab-done'),
    t('t11', 'pharmacy-running', 'done.invoke.pharmacy-running', 'pharmacy-done'),
    t('t12', 'orders', 'done.state.orders', 'consultation'),
    t('t13', 'consultation', 'disposition.decided', 'disposition'),
    t('t14', 'disposition', 'disposition.completed', 'done', { kind: 'none' }, [{ id: 'encounter/complete-encounter', params: {} }]),
  ],
  tables: [
    {
      id: 'triage-route',
      title: 'Route after triage',
      input: '%triage.category',
      rows: [
        { when: "%triage.category = 'emergency'", outcome: 'bypass' },
        { when: "%patient.age() < 5 and %triage.category = 'urgent'", outcome: 'bypass' },
        { when: 'true', outcome: 'normal' },
      ],
    },
  ],
  dataRequirements: ['%triage.category', '%patient.age()', '%gate', '%encounter'],
  principal: ['worklists.manage'],
}

export type Step = { event?: string | undefined; wait?: string | undefined; expect: string[] }
export type Scenario = { name: string; guards: Record<string, boolean>; steps: Step[] }

const emergency = 'table triage-route = bypass'
const gate = "%gate.where(id = 'opd-consultation-fee').active"

export const SCENARIOS: Scenario[] = [
  {
    name: 'Routine visit, paid at the gate, with lab and pharmacy',
    guards: { [emergency]: false, [gate]: true },
    steps: [
      { expect: ['reception'] },
      { event: 'reception.checked-in', expect: ['triage'] },
      { event: 'triage.completed', expect: ['payment'] },
      { event: 'gate.pass', expect: ['consultation', 'assessing'] },
      { event: 'orders.placed', expect: ['orders', 'lab', 'lab-running', 'pharmacy', 'pharmacy-running'] },
      { event: 'done.invoke.lab-running', expect: ['orders', 'lab', 'lab-done', 'pharmacy', 'pharmacy-running'] },
      { event: 'done.invoke.pharmacy-running', expect: ['consultation', 'assessing'] },
      { event: 'disposition.decided', expect: ['disposition'] },
      { event: 'disposition.completed', expect: ['done'] },
    ],
  },
  {
    name: 'Emergency bypasses the fee',
    guards: { [emergency]: true },
    steps: [
      { expect: ['reception'] },
      { event: 'reception.checked-in', expect: ['triage'] },
      { event: 'triage.completed', expect: ['consultation', 'assessing'] },
    ],
  },
  {
    name: 'Triage waits too long',
    guards: {},
    steps: [
      { expect: ['reception'] },
      { event: 'reception.checked-in', expect: ['triage'] },
      { wait: 'PT45M', expect: ['triage'] },
    ],
  },
]

// --- The sandbox stand-in -------------------------------------------------------------------

export type Trace = {
  label: string
  configuration: string[]
  /** Transitions taken in this step, for highlighting the path on the canvas. */
  taken: string[]
  actions: string[]
  tasks: string[]
  timers: string[]
  expected: string[]
  pass: boolean
  error?: string | undefined
}

const minutes = (iso: string) => {
  const m = /^P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?)?$/.exec(iso)
  if (!m) return Number.NaN
  return Number(m[1] ?? 0) * 1440 + Number(m[2] ?? 0) * 60 + Number(m[3] ?? 0)
}

export const guardKey = (g: Guard) => (g.kind === 'fhirpath' ? g.expression : g.kind === 'table' ? `table ${g.table} = ${g.outcome}` : '')
export const guardText = (g: Guard) => (g.kind === 'fhirpath' ? `[${g.expression}]` : g.kind === 'table' ? `[${g.table} → ${g.outcome}]` : '')
export const describeAction = (a: Action) => `${a.id}${Object.keys(a.params).length ? ` (${Object.entries(a.params).map(([k, v]) => `${k}=${v}`).join(', ')})` : ''}`

export function simulate(def: Definition, scenario: Scenario): Trace[] {
  const byId = new Map(def.states.map((x) => [x.id, x]))
  const children = (p: string | null) => def.states.filter((x) => x.parent === p)
  const ancestors = (id: string): string[] => {
    const out: string[] = []
    let at = byId.get(id)?.parent ?? null
    while (at) {
      out.push(at)
      at = byId.get(at)?.parent ?? null
    }
    return out
  }
  const order = def.states.map((x) => x.id)
  const sorted = (set: Set<string>) => order.filter((id) => set.has(id))
  let active = new Set<string>()
  let actions: string[] = []
  let taken: string[] = []
  let elapsed = 0
  let enteredAt = new Map<string, number>()
  const internal: string[] = []

  const enter = (id: string) => {
    const st = byId.get(id)
    if (!st || active.has(id)) return
    active.add(id)
    enteredAt.set(id, elapsed)
    for (const a of st.entry) actions.push(describeAction(a))
    if (st.invoke) actions.push(`invoke ${st.invoke}`)
    if (st.gate) actions.push(`finance/evaluate-gate (${st.gate})`)
    if (st.kind === 'compound') enter(st.initial ?? children(id)[0]?.id ?? '')
    if (st.kind === 'parallel') for (const c of children(id)) enter(c.id)
    if (st.kind === 'final' && st.parent) {
      internal.push(`done.state.${st.parent}`)
      const grand = byId.get(byId.get(st.parent)?.parent ?? '')
      if (grand?.kind === 'parallel') {
        const allDone = children(grand.id).every((region) => children(region.id).some((c) => c.kind === 'final' && active.has(c.id)))
        if (allDone) internal.push(`done.state.${grand.id}`)
      }
    }
  }
  const enterPath = (domain: string | null, target: string) => {
    const path = [target, ...ancestors(target)]
    const stop = domain === null ? path.length : path.indexOf(domain)
    for (const id of path.slice(0, stop === -1 ? path.length : stop).slice().reverse()) {
      if (id === target) enter(id)
      else if (!active.has(id)) {
        active.add(id)
        enteredAt.set(id, elapsed)
      }
    }
    // A parallel ancestor entered on the way must enter its other regions too.
    for (const id of path) {
      const st = byId.get(id)
      if (st?.kind === 'parallel') for (const c of children(id)) if (!active.has(c.id)) enter(c.id)
    }
  }
  const domainOf = (from: string, to: string): string | null => {
    const up = ancestors(from)
    if (from === to) return byId.get(from)?.parent ?? null
    const toUp = new Set([to, ...ancestors(to)])
    for (const a of up) if (toUp.has(a) && byId.get(a)?.kind === 'compound') return a
    return null
  }
  const take = (tr: Transition) => {
    const domain = domainOf(tr.from, tr.to)
    const inside = (id: string) => domain === null || ancestors(id).includes(domain)
    active = new Set([...active].filter((id) => !inside(id)))
    enteredAt = new Map([...enteredAt].filter(([id]) => active.has(id)))
    for (const a of tr.actions) actions.push(describeAction(a))
    taken.push(tr.id)
    enterPath(domain, tr.to)
  }
  const enabled = (event: string): Transition[] => {
    const picked: Transition[] = []
    const leaves = sorted(active).filter((id) => !children(id).some((c) => active.has(c.id)))
    for (const leaf of leaves) {
      for (const source of [leaf, ...ancestors(leaf)]) {
        const found = def.transitions.find((tr) => {
          if (tr.from !== source || tr.event !== event) return false
          if (tr.guard.kind === 'none') return true
          const key = guardKey(tr.guard)
          if (!(key in scenario.guards)) throw new Error(`Guard not stubbed: ${key}`)
          return scenario.guards[key] === true
        })
        if (found) {
          if (!picked.includes(found)) picked.push(found)
          break
        }
      }
    }
    return picked
  }
  const settle = () => {
    // Run to completion: eventless transitions first, then the internal (done.state) events.
    for (let n = 0; n < 50; n++) {
      const eventless = enabled('')
      if (eventless.length > 0) {
        for (const tr of eventless) take(tr)
        continue
      }
      const ev = internal.shift()
      if (ev === undefined) return
      for (const tr of enabled(ev)) take(tr)
    }
  }
  const pendingTimers = () =>
    def.transitions
      .filter((tr) => tr.event.startsWith('after ') && active.has(tr.from))
      .map((tr) => ({ tr, due: (enteredAt.get(tr.from) ?? 0) + minutes(tr.event.slice(6)) }))

  const traces: Trace[] = []
  for (const [n, step] of scenario.steps.entries()) {
    actions = []
    taken = []
    let error: string | undefined
    try {
      if (n === 0 && step.event === undefined && step.wait === undefined) {
        enter(def.initial)
        settle()
      } else if (step.event !== undefined) {
        for (const tr of enabled(step.event)) take(tr)
        settle()
      } else if (step.wait !== undefined) {
        const until = elapsed + minutes(step.wait)
        for (const timer of [...pendingTimers()].sort((a, b) => a.due - b.due)) {
          if (timer.due <= until && active.has(timer.tr.from)) {
            elapsed = timer.due
            take(timer.tr)
            settle()
          }
        }
        elapsed = until
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
    const configuration = sorted(active)
    const tasks = configuration.flatMap((id) => {
      const st = byId.get(id)
      return st?.task ? [`${st.label}: ${st.task.role} on ${st.task.worklist} (${st.task.priority})`] : st?.gate ? [`${st.label}: cashier, if blocked`] : []
    })
    const timers = pendingTimers().map(({ tr, due }) => `${tr.event} on ${byId.get(tr.from)?.label ?? tr.from} (in ${due - elapsed} min)`)
    const pass = error === undefined && configuration.length === step.expect.length && step.expect.every((id) => active.has(id))
    traces.push({ label: step.event ?? (step.wait ? `wait ${step.wait}` : 'start'), configuration, taken, actions, tasks, timers, expected: step.expect, pass, error })
  }
  return traces
}

// --- Publication checks (workflow-engine.md, "Simulation and publication gates") --------------

export type Check = { title: string; problems: { text: string; state?: string | undefined; transition?: string | undefined }[] }

export function checks(def: Definition, scenarios: Scenario[] = SCENARIOS): Check[] {
  const byId = new Map(def.states.map((x) => [x.id, x]))
  const children = (p: string | null) => def.states.filter((x) => x.parent === p)
  const ancestors = (id: string): string[] => {
    const out: string[] = []
    let at = byId.get(id)?.parent ?? null
    while (at) {
      out.push(at)
      at = byId.get(at)?.parent ?? null
    }
    return out
  }

  // Reachability: from the initial state, through transitions, initial children and parallel regions.
  const reached = new Set<string>()
  const visit = (id: string) => {
    if (reached.has(id) || !byId.has(id)) return
    reached.add(id)
    for (const a of ancestors(id)) reached.add(a)
    const st = byId.get(id)
    if (st?.kind === 'compound') visit(st.initial ?? children(id)[0]?.id ?? '')
    if (st?.kind === 'parallel') for (const c of children(id)) visit(c.id)
    for (const tr of def.transitions) if (tr.from === id || ancestors(id).includes(tr.from)) visit(tr.to)
  }
  visit(def.initial)
  const unreachable = def.states.filter((x) => !reached.has(x.id)).map((x) => ({ text: `${x.label} can never be entered`, state: x.id }))

  const exits = def.states
    .filter((x) => x.kind === 'atomic' && !def.transitions.some((tr) => tr.from === x.id || ancestors(x.id).includes(tr.from)))
    .map((x) => ({ text: `${x.label} is not final and has no way out`, state: x.id }))

  const actionProblems: Check['problems'] = []
  const everyAction = [...def.states.flatMap((x) => x.entry.map((a) => ({ a, state: x.id, transition: undefined as string | undefined }))), ...def.transitions.flatMap((tr) => tr.actions.map((a) => ({ a, state: undefined as string | undefined, transition: tr.id })))]
  for (const { a, state, transition } of everyAction) {
    const known = ACTIONS.find((x) => x.id === a.id)
    if (!known) {
      actionProblems.push({ text: `Unknown action ${a.id}`, state, transition })
      continue
    }
    for (const p of known.params) {
      const v = a.params[p.name]
      if (p.required && !v) actionProblems.push({ text: `${known.title}: ${p.name} is required`, state, transition })
      if (v && p.values && !p.values.includes(v)) actionProblems.push({ text: `${known.title}: ${p.name} must be one of ${p.values.join(', ')}`, state, transition })
    }
  }
  for (const x of def.states) if (x.kind === 'compound' && x.initial && byId.get(x.initial)?.parent !== x.id) actionProblems.push({ text: `${x.label} starts in a state that is not inside it`, state: x.id })

  const guardProblems: Check['problems'] = []
  for (const tr of def.transitions) {
    if (tr.guard.kind === 'fhirpath') {
      for (const ref of tr.guard.expression.match(/%[a-zA-Z]+/g) ?? []) {
        if (!def.dataRequirements.some((d) => d.startsWith(ref))) guardProblems.push({ text: `${ref} is not in the data requirements`, transition: tr.id })
      }
    }
    const g = tr.guard
    if (g.kind === 'table') {
      const table = def.tables.find((x) => x.id === g.table)
      if (!table) guardProblems.push({ text: `No decision table ${g.table}`, transition: tr.id })
      else if (!table.rows.some((r) => r.outcome === g.outcome)) guardProblems.push({ text: `${table.title} never gives ${g.outcome}`, transition: tr.id })
    }
  }

  const principalProblems: Check['problems'] = []
  for (const tr of def.transitions) {
    if (!(tr.event === '' || tr.event.startsWith('after '))) continue
    for (const a of tr.actions) {
      const known = ACTIONS.find((x) => x.id === a.id)
      if (known && !def.principal.includes(known.permission)) principalProblems.push({ text: `${tr.event || 'An eventless transition'} runs ${known.title}, which needs ${known.permission}; the workflow principal does not hold it`, transition: tr.id })
    }
  }

  const scenarioProblems = scenarios.flatMap((sc) =>
    simulate(def, sc)
      .map((tr, n) => ({ tr, n }))
      .filter(({ tr }) => !tr.pass)
      .slice(0, 1)
      .map(({ tr, n }) => ({ text: `“${sc.name}”, step ${n + 1} (${tr.label}): ${tr.error ?? `expected ${tr.expected.join(', ')}, got ${tr.configuration.join(', ') || 'nothing'}`}` })),
  )

  return [
    { title: 'Every scenario passes', problems: scenarioProblems },
    { title: 'Every state can be entered', problems: unreachable },
    { title: 'Every state that is not final has a way out', problems: exits },
    { title: 'Every action and its parameters are valid', problems: actionProblems },
    { title: 'Every guard reads only declared data', problems: guardProblems },
    { title: 'The workflow principal holds what timers and eventless steps need', problems: principalProblems },
  ]
}
