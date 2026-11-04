import { describe, expect, test } from 'vitest'

import { WorkflowError, claim, complete, definitionsQuery, release, startInstance, worklistQuery, worklists, type WorkTask } from '../src/features/worklists/api'
import { fakeTransport } from './fake'

const task: WorkTask = {
  id: 't1',
  instance: 'i 1',
  definition: 'urn:opd|1.0.0',
  context: 'Encounter/e-1',
  state: 'triage',
  status: 'ready',
  role: 'nurse',
  form: null,
  worklist: 'opd-triage',
  priority: 'routine',
  owner: null,
  authoredOn: '2026-11-04T08:00:00Z',
  lastModified: '2026-11-04T08:00:00Z',
  events: ['triaged'],
}

describe('worklists', () => {
  test('the worklists are those the definitions name, each once', async () => {
    const transport = fakeTransport(() => ({
      body: [
        { definition: 'urn:opd|1.0.0', tasks: [{ state: 'triage', role: 'nurse', worklist: 'opd-triage' }, { state: 'consultation', role: 'clinician', worklist: 'opd-consult' }] },
        { definition: 'urn:anc|1.0.0', tasks: [{ state: 'visit', role: 'midwife', worklist: 'opd-triage' }] },
      ],
    }))
    const listed = await definitionsQuery(transport).queryFn()
    expect(transport.sent[0]).toMatchObject({ method: 'GET', path: '/workflow/definitions' })
    expect(worklists(listed)).toEqual(['opd-consult', 'opd-triage'])
  })

  test('a worklist is read by its key', async () => {
    const transport = fakeTransport(() => ({ body: [task] }))
    await expect(worklistQuery(transport, 'opd triage').queryFn()).resolves.toEqual([task])
    expect(transport.sent[0]).toMatchObject({ path: '/workflow/worklists/opd%20triage' })
  })

  test('commands carry an op id and name the task by instance and state', async () => {
    const transport = fakeTransport((r) => ({ status: r.path === '/workflow/instances' ? 201 : 200, body: {} }))
    await startInstance(transport, 'urn:opd|1.0.0', 'Encounter/e-1')
    await claim(transport, task)
    await release(transport, task)
    await complete(transport, task, 'triaged')
    expect(transport.sent.map((r) => r.path)).toEqual([
      '/workflow/instances',
      '/workflow/instances/i%201/tasks/triage/claim',
      '/workflow/instances/i%201/tasks/triage/release',
      '/workflow/instances/i%201/events',
    ])
    expect(transport.sent[0]?.body).toMatchObject({ definition: 'urn:opd|1.0.0', context: 'Encounter/e-1' })
    expect(transport.sent[3]?.body).toMatchObject({ event: 'triaged' })
    const ops = transport.sent.map((r) => (r.body as { opId: string }).opId)
    expect(new Set(ops).size).toBe(4)
  })

  test('a refusal carries its status and reason', async () => {
    const transport = fakeTransport(() => ({ status: 409, body: { error: 'the task is claimed by nurse-2' } }))
    await expect(claim(transport, task)).rejects.toEqual(new WorkflowError(409, 'the task is claimed by nurse-2'))
  })
})
