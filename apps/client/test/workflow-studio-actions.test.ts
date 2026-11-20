import { describe, expect, test } from 'vitest'

import libraryJson from '../../../packages/reference-packages/opd/Library-opd-visit.json'
import {
  addExecute,
  declarePermission,
  edgeId,
  entryActions,
  flagged,
  parameterFields,
  problemAt,
  problems,
  readDefinition,
  stateOf,
  transitionAt,
  undeclarePermission,
  updateState,
  updateTransition,
  withEntryActions,
  withParam,
  withReference,
  withTrigger,
  type ActionSpec,
  type WorkflowDefinition,
} from '../src/features/studio/workflows/model'

// The Workflow Studio's typed actions (#498, part 2): picked from the release's catalogue, with
// their parameters generated from their schemas, checked as `WorkflowDefinition::problems` checks
// them, and the workflow principal's permissions declared from the checks.

const read = readDefinition(JSON.stringify(libraryJson))
if (!('definition' in read)) throw new Error('the OPD definition does not read')
const opd = read.definition

const queue: ActionSpec = {
  reference: 'patient-flow/open-queue-entry@1',
  id: 'patient-flow/open-queue-entry',
  major: 1,
  permission: 'queues.work',
  compensation: { kind: 'action', action: 'patient-flow/close-queue-entry' },
  parameterSchema: {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'patient-flow/open-queue-entry@1',
    type: 'object',
    properties: { service_point: { type: 'string', minLength: 1 }, priority: { type: 'string', enum: ['routine', 'urgent'] }, count: { type: 'integer' }, notify: { type: 'boolean' } },
    required: ['service_point', 'notify'],
    additionalProperties: false,
  },
}
const specimen: ActionSpec = {
  reference: 'laboratory/collect-specimen@2',
  id: 'laboratory/collect-specimen',
  major: 2,
  permission: 'laboratory.work',
  compensation: { kind: 'none', action: null },
  parameterSchema: { $schema: 'https://json-schema.org/draft/2020-12/schema', title: 'laboratory/collect-specimen@2', type: 'object', properties: {}, required: [], additionalProperties: false },
}
const catalogue = [queue, specimen]
const withoutPrincipal = (d: WorkflowDefinition): WorkflowDefinition => ({ ...d, principal: [] })

/** OPD with `reception`'s first transition executing `params` of `reference`. */
function executing(reference: string, params: Record<string, boolean | number | string> = {}): WorkflowDefinition {
  return updateTransition(opd, 'reception', 0, (t) => ({ ...t, actions: [{ kind: 'execute', action: reference, params }] }))
}

describe('typed actions in the Workflow Studio', () => {
  test('the parameter fields come from the schema: kind, codes and whether required', () => {
    expect(parameterFields(queue)).toEqual([
      { name: 'count', type: 'integer', codes: null, required: false },
      { name: 'notify', type: 'boolean', codes: null, required: true },
      { name: 'priority', type: 'string', codes: ['routine', 'urgent'], required: false },
      { name: 'service_point', type: 'string', codes: null, required: true },
    ])
  })

  test('an action picked from the catalogue starts with its required flags, numbers and codes filled', () => {
    expect(addExecute([], queue)).toEqual([{ kind: 'execute', action: 'patient-flow/open-queue-entry@1', params: { notify: false } }])
    expect(addExecute([{ kind: 'raise', event: 'x' }], specimen)).toEqual([
      { kind: 'raise', event: 'x' },
      { kind: 'execute', action: 'laboratory/collect-specimen@2', params: {} },
    ])
  })

  test('a parameter is set, cleared, and a picked action keeps only the parameters it has', () => {
    const [action] = addExecute([], queue)
    if (action?.kind !== 'execute') throw new Error('not an execute')
    const set = withParam(withParam(action, 'service_point', 'opd-1'), 'count', 3)
    expect(set.params).toEqual({ notify: false, service_point: 'opd-1', count: 3 })
    expect(withParam(set, 'count', undefined).params).toEqual({ notify: false, service_point: 'opd-1' })
    expect(withReference(set, specimen)).toEqual({ kind: 'execute', action: 'laboratory/collect-specimen@2', params: {} })
  })

  test('the checks find what publication would: references, unknown actions and each parameter problem', () => {
    const at = { kind: 'transition', source: 'reception', index: 0 } as const
    expect(problems(withoutPrincipal(executing('patient-flow/open-queue-entry@1', { service_point: 'opd', notify: true })), catalogue)).toEqual([])
    expect(problems(executing('open-queue-entry'), catalogue)).toEqual([{ code: 'bad-reference', at, action: 'open-queue-entry' }])
    expect(problems(executing('patient-flow/open-queue-entry@2'), catalogue)).toEqual([{ code: 'unknown-action', at, action: 'patient-flow/open-queue-entry@2' }])
    expect(problems(executing('patient-flow/open-queue-entry@1', { notify: 'yes', priority: 'stat', service_point: '', colour: 'red' }), catalogue)).toEqual([
      { code: 'unknown-parameter', at, action: 'patient-flow/open-queue-entry@1', param: 'colour' },
      { code: 'invalid-parameter', at, action: 'patient-flow/open-queue-entry@1', param: 'notify' },
      { code: 'invalid-parameter', at, action: 'patient-flow/open-queue-entry@1', param: 'priority' },
      { code: 'invalid-parameter', at, action: 'patient-flow/open-queue-entry@1', param: 'service_point' },
    ])
    expect(problems(executing('patient-flow/open-queue-entry@1', { count: 1.5 }), catalogue)).toEqual([
      { code: 'invalid-parameter', at, action: 'patient-flow/open-queue-entry@1', param: 'count' },
      { code: 'missing-parameter', at, action: 'patient-flow/open-queue-entry@1', param: 'notify' },
      { code: 'missing-parameter', at, action: 'patient-flow/open-queue-entry@1', param: 'service_point' },
    ])
    // Each selects the transition, and outlines it.
    const found = problems(executing('patient-flow/open-queue-entry@2'), catalogue)
    expect(found.map(problemAt)).toEqual([at])
    expect(flagged(found)).toEqual(new Set([edgeId('reception', 0)]))
    // Without the catalogue (not loaded yet), actions are not checked.
    expect(problems(executing('patient-flow/open-queue-entry@2'))).toEqual([])
  })

  test('entry actions are checked at their state', () => {
    const entered = updateState(opd, 'triage', (s) => withEntryActions(s, [{ kind: 'execute', action: 'nowhere/thing@1' }]))
    expect(entryActions(stateOf(entered, 'triage') ?? { id: '', kind: 'atomic' })).toHaveLength(1)
    expect(problems(entered, catalogue)).toEqual([{ code: 'unknown-action', at: { kind: 'state', id: 'triage' }, action: 'nowhere/thing@1' }])
  })

  test('a timer or eventless step needs the principal to hold its actions’ permissions, and it can be declared from the check', () => {
    const at = { kind: 'transition', source: 'reception', index: 0 } as const
    const params = { service_point: 'opd', notify: true }
    // A user's event: the user's own permission is checked when it runs, so nothing is needed.
    expect(problems(executing(queue.reference, params), catalogue)).toEqual([])
    // A timer runs as the workflow principal.
    const timed = withTrigger(executing(queue.reference, params), 'reception', 0, { kind: 'timer', after: 'PT2H', event: '' })
    expect(problems(timed, catalogue)).toEqual([{ code: 'principal-lacks', at, permission: 'queues.work', action: queue.reference }])
    // So does an eventless step, including the entry actions of the state it enters.
    const target = transitionAt(opd, 'reception', 0)?.targets?.[0] ?? ''
    const eventless = updateState(withTrigger(opd, 'reception', 0, { kind: 'eventless' }), target, (s) => withEntryActions(s, [{ kind: 'execute', action: specimen.reference }]))
    expect(problems(eventless, catalogue)).toEqual([{ code: 'principal-lacks', at, permission: 'laboratory.work', action: specimen.reference }])

    const declared = declarePermission(timed, 'queues.work')
    expect(declared.principal).toEqual(['queues.work'])
    expect(declarePermission(declared, 'queues.work').principal).toEqual(['queues.work'])
    expect(problems(declared, catalogue)).toEqual([])
  })

  test('a principal permission no action needs is flagged, and can be taken back', () => {
    const extra = declarePermission(opd, 'stock.manage')
    expect(problems(extra, catalogue)).toEqual([{ code: 'unneeded-permission', permission: 'stock.manage' }])
    expect(problemAt({ code: 'unneeded-permission', permission: 'stock.manage' })).toBeNull()
    expect(undeclarePermission(extra, 'stock.manage').principal).toBeUndefined()
  })
})
