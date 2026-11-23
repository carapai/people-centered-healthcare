import { describe, expect, test } from 'vitest'

import libraryJson from '../../../packages/reference-packages/opd/Library-opd-visit.json'
import routeJson from '../../../packages/reference-packages/opd/Library-opd-triage-route.json'
import { arrange, GROUP_HEADER, type Box } from '../src/features/studio/workflows/layout'
import {
  activityOf,
  addState,
  addTransition,
  competitors,
  edgeId,
  flagged,
  guardLabel,
  moveSource,
  moveState,
  moveTransition,
  problemAt,
  problems,
  readDefinition,
  removeState,
  removeTransition,
  renameState,
  rows,
  stateOf,
  tableOutcomes,
  transitionAt,
  triggerOf,
  updateState,
  updateTransition,
  withActivity,
  withEntryActions,
  withTrigger,
  writeDefinition,
  type WorkflowDefinition,
} from '../src/features/studio/workflows/model'

// The Workflow Studio's model and layout (#498): the OPD reference definition read from its
// Library, laid out, checked and edited as the outline, canvas and inspector do.

const read = readDefinition(JSON.stringify(libraryJson))
if (!('definition' in read)) throw new Error('the OPD definition does not read')
const { library, definition: opd } = read

const overlap = (a: Box, b: Box) => a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height

describe('the Workflow Studio', () => {
  test('the OPD reference definition reads from its Library and writes back unchanged', () => {
    expect(rows(opd).map((r) => r.state.id)).toEqual(['reception', 'triage', 'payment', 'consultation', 'orders', 'lab', 'lab-awaiting', 'lab-back', 'pharmacy', 'pharmacy-awaiting', 'pharmacy-back', 'review', 'disposition', 'discharged', 'admitted', 'referred'])
    const written = writeDefinition(library, opd)
    expect({ ...written, content: undefined }).toEqual({ ...library, content: undefined })
    const again = readDefinition(JSON.stringify(written))
    expect('definition' in again && again.definition).toEqual(opd)
    expect(readDefinition(JSON.stringify({ resourceType: 'Library', type: { coding: [{ code: 'logic-library' }] } }))).toEqual({ refusal: 'not-workflow' })
    expect(readDefinition('{')).toEqual({ refusal: 'not-json' })
  })

  test('the OPD reference definition lays out with no overlapping nodes', () => {
    const boxes = arrange(opd)
    expect(new Set(boxes.keys())).toEqual(new Set(rows(opd).map((r) => r.state.id)))
    const all = rows(opd)
    for (const container of [null, ...all.map((r) => r.state.id)]) {
      const siblings = all.filter((r) => r.parent === container).map((r) => ({ id: r.state.id, box: boxes.get(r.state.id) as Box }))
      for (const [n, a] of siblings.entries()) for (const b of siblings.slice(n + 1)) expect(overlap(a.box, b.box), `${a.id} and ${b.id} overlap`).toBe(false)
      // A group holds its children, below its title.
      if (container !== null && siblings.length > 0) {
        const group = boxes.get(container) as Box
        for (const { id, box } of siblings) {
          expect(box.x, id).toBeGreaterThanOrEqual(0)
          expect(box.y, id).toBeGreaterThanOrEqual(GROUP_HEADER)
          expect(box.x + box.width, id).toBeLessThanOrEqual(group.width)
          expect(box.y + box.height, id).toBeLessThanOrEqual(group.height)
        }
      }
    }
    // Left to right: reception comes before disposition.
    expect((boxes.get('reception') as Box).x).toBeLessThan((boxes.get('disposition') as Box).x)
  })

  test('the OPD reference definition passes the checks the Studio runs', () => {
    expect(problems(opd)).toEqual([])
  })

  test('an unreachable state is listed, outlined, and its problem selects it; a transition to it clears it', () => {
    const added = addState(opd, null)
    expect(added.id).toBe('state')
    const found = problems(added.definition)
    expect(found).toEqual([
      { code: 'unreachable', state: 'state' },
    ])
    expect(problemAt(found[0] as (typeof found)[number])).toEqual({ kind: 'state', id: 'state' })
    expect(flagged(found).has('state')).toBe(true)
    // Reached from consultation, it has no way out, until it has one.
    const reached = addTransition(added.definition, 'consultation', 'state')
    expect(reached.index).toBe(2)
    expect(transitionAt(reached.definition, 'consultation', 2)).toEqual({ events: ['consultation.to-state'], targets: ['state'] })
    expect(problems(reached.definition)).toEqual([{ code: 'dead-end', state: 'state' }])
    const out = addTransition(reached.definition, 'state', 'discharged')
    expect(problems(out.definition)).toEqual([])
  })

  test('a state inside a group is added there, moved out, renamed with its references, and removed with the transitions only to it', () => {
    const inside = addState(opd, 'lab')
    expect(rows(inside.definition).find((r) => r.state.id === inside.id)?.parent).toBe('lab')
    const out = moveState(inside.definition, inside.id, null)
    expect(rows(out).find((r) => r.state.id === inside.id)).toMatchObject({ parent: null, depth: 0 })
    // Not into itself.
    expect(moveState(opd, 'orders', 'lab')).toBe(opd)

    const renamed = renameState(opd, 'orders', 'investigations')
    expect(transitionAt(renamed, 'consultation', 0)?.targets).toEqual(['investigations'])
    expect(transitionAt(renamed, 'investigations', 0)?.events).toEqual(['done.state.investigations'])
    expect(problems(renamed)).toEqual([])

    const removed = removeState(opd, 'review')
    expect(stateOf(removed, 'review')).toBeUndefined()
    // orders only went to review, on done.state.orders: that transition goes.
    expect(stateOf(removed, 'orders')?.transitions).toBeUndefined()
    expect(problems(removed)).toEqual([])
  })

  test('a human task, a gate and the checks on them', () => {
    expect(activityOf(stateOf(opd, 'triage') as never)).toBe('task')
    expect(activityOf(stateOf(opd, 'payment') as never)).toBe('gate')
    expect(activityOf(stateOf(opd, 'reception') as never)).toBe('wait')
    // A task needs a role and a worklist, and its form is url|version.
    const noForm = updateState(opd, 'triage', (s) => ({ ...s, task: { role: 'nurse', worklist: 'opd-triage', form: 'https://health.go.ug/fhir/Questionnaire/opd-triage' } }))
    expect(problems(noForm)).toEqual([{ code: 'bad-task', state: 'triage' }])
    // A gate moves on at gate.pass and gate.defer.
    const index = (stateOf(opd, 'payment')?.transitions ?? []).findIndex((t) => t.events?.includes('gate.defer'))
    expect(problems(removeTransition(opd, 'payment', index))).toEqual([{ code: 'bad-gate', state: 'payment' }])
    // A new gate's task is the cashier's.
    expect(withActivity({ id: 'pay', kind: 'atomic' }, 'gate')).toEqual({ id: 'pay', kind: 'atomic', gate: '', task: { role: 'cashier', worklist: 'cashier' } })
    expect(withActivity(stateOf(opd, 'triage') as never, 'wait')).not.toHaveProperty('task')
  })

  test('triggers: an event, a timer of the source state, or none; competitors in document order', () => {
    const triage = stateOf(opd, 'triage')
    if (triage === undefined) throw new Error('no triage')
    expect(triggerOf(triage, triage.transitions?.[0] ?? {})).toEqual({ kind: 'event', events: ['opd.triaged-routine'] })
    expect(competitors(triage, 0)).toEqual([])

    const timed = withTrigger(opd, 'triage', 0, { kind: 'timer', after: 'PT1H', event: '' })
    const state = stateOf(timed, 'triage')
    expect(state?.timers).toEqual([{ after: 'PT1H', event: 'triage.timeout' }])
    expect(triggerOf(state as never, state?.transitions?.[0] ?? {})).toEqual({ kind: 'timer', after: 'PT1H', event: 'triage.timeout' })
    // Back to an event, the timer goes.
    const back = withTrigger(timed, 'triage', 0, { kind: 'event', events: ['opd.triaged-routine'] })
    expect(stateOf(back, 'triage')?.timers).toBeUndefined()
    expect(triggerOf(stateOf(back, 'triage') as never, { events: [] })).toEqual({ kind: 'eventless' })

    // A second transition on the same event competes; the first whose guard holds wins.
    const second = updateTransition(addTransition(opd, 'triage', 'consultation').definition, 'triage', 3, (t) => ({ ...t, events: ['opd.triaged-routine'], guard: "%urgency = 'routine'" }))
    expect(competitors(stateOf(second, 'triage') as never, 3).map((c) => c.index)).toEqual([0])
    const moved = moveTransition(second, 'triage', 3, -1)
    expect(moved.index).toBe(2)
    expect(transitionAt(moved.definition, 'triage', 2)?.guard).toBe("%urgency = 'routine'")
    // Moved to leave another state, last there.
    const elsewhere = moveSource(second, 'triage', 3, 'reception')
    expect(elsewhere.index).toBe(2)
    expect(transitionAt(elsewhere.definition, 'reception', 2)?.targets).toEqual(['consultation'])
    expect(edgeId('reception', 2)).toBe('reception→2')
  })

  test('an assignment to an undeclared field, or of another kind, is a problem on its element', () => {
    const bad = updateTransition(opd, 'reception', 1, (t) => ({ ...t, actions: [{ kind: 'assign', field: 'urgency', value: true }] }))
    expect(problems(bad)).toEqual([{ code: 'bad-assignment', at: { kind: 'transition', source: 'reception', index: 1 }, field: 'urgency' }])
    const entry = updateState(opd, 'triage', (s) => withEntryActions(s, [{ kind: 'assign', field: 'seen', value: 'yes' }]))
    expect(stateOf(entry, 'triage')?.on_entry).toEqual([[{ kind: 'assign', field: 'seen', value: 'yes' }]])
    expect(problems(entry)).toEqual([{ code: 'bad-assignment', at: { kind: 'state', id: 'triage' }, field: 'seen' }])
  })

  test('a compound or parallel state needs children; its initial child is one of them', () => {
    const empty: WorkflowDefinition = { url: 'u', version: '1', statechart: { states: [{ id: 'a', kind: 'compound' }] } }
    expect(problems(empty)).toEqual([{ code: 'bad-children', state: 'a' }])
    const bad = updateState(opd, 'lab', (s) => ({ ...s, initial: ['pharmacy-awaiting'] }))
    expect(problems(bad)).toContainEqual({ code: 'bad-initial', state: 'lab' })
  })

  test('a guard may name a decision table and the outcome it requires, checked as publication does (ADR 0093)', () => {
    const route = readDefinition(JSON.stringify(routeJson))
    if (!('definition' in route)) throw new Error('the triage route does not read')
    const definition = route.definition
    const [table] = definition.decisionTables ?? []
    if (table === undefined) throw new Error('no table')
    expect(tableOutcomes(table)).toEqual(['emergency', 'urgent', 'routine'])
    expect(guardLabel(transitionAt(definition, 'triage', 0)?.guard)).toBe('[triage-priority = emergency]')
    expect(guardLabel('%urgent')).toBe('[%urgent]')
    expect(guardLabel(null)).toBe('')
    expect(problems(definition)).toEqual([])
    const guarded = (guard: { table: string; outcome: string }) => updateTransition(definition, 'triage', 0, (t) => ({ ...t, guard }))
    const at = { kind: 'transition', source: 'triage', index: 0 }
    expect(problems(guarded({ table: 'danger', outcome: 'emergency' }))).toEqual([{ code: 'unknown-guard-table', at, table: 'danger' }])
    expect(problems(guarded({ table: 'triage-priority', outcome: 'admit' }))).toEqual([{ code: 'impossible-guard-outcome', at, table: 'triage-priority', outcome: 'admit' }])
    // The default is an outcome the table gives.
    expect(problems(guarded({ table: 'triage-priority', outcome: 'routine' }))).toEqual([])
    expect(flagged(problems(guarded({ table: 'danger', outcome: 'x' })))).toEqual(new Set([edgeId('triage', 0)]))
    // It writes back to its Library unchanged.
    const again = readDefinition(JSON.stringify(writeDefinition(route.library, definition)))
    expect('definition' in again && again.definition).toEqual(definition)
  })
})
