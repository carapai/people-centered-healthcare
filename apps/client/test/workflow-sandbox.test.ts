import { describe, expect, test } from 'vitest'

import type { StudioSandbox } from '../src/features/studio/workflows/api'
import { activeStates, answer, current, emptyRun, restart, scenarioFile, scenarioPath, send, setGate, setTableOutcome, undo, untaken, wait } from '../src/features/studio/workflows/sandbox'

// The Workflow Studio's sandbox (#499): the browser keeps the author's answers and moves; every
// step, what is active and the scenario file come from the server's answer.

const step = (configuration: string[]) => ({ configuration, actions: [], timers: [], tasks: [], running: true, accepts: [], elapsed: 'PT0S' })

const played = (steps: string[][], unanswered: string[] = []): StudioSandbox => ({
  guards: ['%urgent'],
  gates: [],
  tables: [],
  steps: steps.map(step),
  unanswered,
  unansweredTables: [],
  refused: null,
  scenario: JSON.stringify({ type: 'workflow-scenario', name: 'Sandbox run', definition: 'http://x|1.0.0', guards: { '%urgent': false }, steps: [{ expect: { configuration: ['triage'] } }] }),
})

describe('the sandbox run', () => {
  test('moves are appended, undone one at a time and dropped on restart, keeping the answers', () => {
    let run = answer(emptyRun(), '%urgent', false)
    run = send(run, 'triaged')
    run = wait(run, 'PT1H')
    expect(run.moves).toEqual([{ event: 'triaged' }, { wait: 'PT1H' }])
    expect(undo(run).moves).toEqual([{ event: 'triaged' }])
    expect(restart(run)).toEqual({ guards: { '%urgent': false }, tables: {}, gates: {}, moves: [] })
    expect(answer(run, '%urgent', null).guards).toEqual({})
    expect(setGate(run, 'http://gate', 'block').gates).toEqual({ 'http://gate': 'block' })
    expect(setGate(setGate(run, 'http://gate', 'block'), 'http://gate', null).gates).toEqual({})
  })

  test('a decision table a guard names is answered by its outcome, kept across restarts (ADR 0093)', () => {
    const run = send(setTableOutcome(emptyRun(), 'triage-priority', 'urgent'), 'opd.triaged')
    expect(run.tables).toEqual({ 'triage-priority': 'urgent' })
    expect(restart(run).tables).toEqual({ 'triage-priority': 'urgent' })
    expect(setTableOutcome(run, 'triage-priority', null).tables).toEqual({})
  })

  test('what is active and how far the run got come from the server', () => {
    const run = send(send(emptyRun(), 'triaged'), 'seen')
    // The server took the first move and stopped at the second: a guard needs an answer.
    const answered = played([['triage'], ['consultation']], ['%urgent'])
    expect(untaken(run, answered)).toBe(1)
    expect(current(answered)?.configuration).toEqual(['consultation'])
    expect(activeStates(answered)).toEqual(new Set(['consultation']))
    // Nothing started: every move waits.
    expect(untaken(run, played([], ['%urgent']))).toBe(2)
    expect(activeStates(undefined)).toEqual(new Set())
  })

  test('a scenario keeps the server’s steps under the author’s name, at a path of its own', () => {
    const file = JSON.parse(scenarioFile(played([['triage']]), '  Routine visit ')) as Record<string, unknown>
    expect(file['name']).toBe('Routine visit')
    expect(file['type']).toBe('workflow-scenario')
    expect(file['steps']).toEqual([{ expect: { configuration: ['triage'] } }])
    expect(scenarioPath('Routine visit', new Set())).toBe('tests/routine-visit.json')
    expect(scenarioPath('Routine visit', new Set(['tests/routine-visit.json']))).toBe('tests/routine-visit-2.json')
    expect(scenarioPath('!!', new Set())).toBe('tests/scenario.json')
  })
})
