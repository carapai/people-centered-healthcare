import type { Transport } from '../../platform/types'

// The laboratory (#109): `/api/laboratory`. The node checks permissions, the order of steps and
// who may verify; a clinician sees results only once verified.

export type Stage = 'ordered' | 'collected' | 'resulted' | 'verified' | 'reviewed' | 'rejected' | 'cancelled'
export type Flag = 'normal' | 'low' | 'high' | 'critical-low' | 'critical-high' | 'abnormal' | 'critical'

export interface ResultValue {
  kind: 'quantity' | 'coded' | 'text'
  value?: string
  unit?: string
  concept?: { text: string }
  text?: string
}

export interface ResultEntry {
  code: { text: string }
  value: ResultValue
  reference?: { low?: string; high?: string } | null
  critical?: { low?: string; high?: string } | null
}

export interface LabTest {
  id: string
  patient: string
  test: { text: string }
  stage: Stage
  results: { entry: ResultEntry; flag: Flag }[]
  reason: string | null
}

/** A request the node refused, with its reason. */
export class LabError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`laboratory request failed: ${status}`)
    this.name = 'LabError'
  }
}

async function call<T>(transport: Transport, method: 'GET' | 'POST', path: string, body?: unknown): Promise<T> {
  const response = await transport.request({ method, path: `/laboratory${path}`, ...(body === undefined ? {} : { body }) })
  if (response.status !== 200) {
    const reason = response.body as { error?: unknown } | undefined
    throw new LabError(response.status, typeof reason?.error === 'string' ? reason.error : '')
  }
  return response.body as T
}

export const testKey = (id: string) => ['laboratory', 'test', id] as const

export const getTest = (transport: Transport, id: string) => call<LabTest>(transport, 'GET', `/tests/${encodeURIComponent(id)}`)

export type Step =
  { step: 'collect'; specimen: string; kind: string } | { step: 'result'; results: ResultEntry[] } | { step: 'verify' } | { step: 'review' } | { step: 'reject'; reason: string }

export function takeStep(transport: Transport, id: string, step: Step): Promise<LabTest> {
  const path = `/tests/${encodeURIComponent(id)}/${step.step}`
  switch (step.step) {
    case 'collect':
      return call(transport, 'POST', path, { specimen: step.specimen, kind: step.kind })
    case 'result':
      return call(transport, 'POST', path, { results: step.results })
    case 'reject':
      return call(transport, 'POST', path, { reason: step.reason })
    default:
      return call(transport, 'POST', path)
  }
}

/** The test a workflow task is about: its ServiceRequest's id. */
export const testOf = (context: string) => context.replace(/^ServiceRequest\//, '')
