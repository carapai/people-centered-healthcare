import type { Transport } from '../../platform/types'

/** Stock held for a dispense being prepared (ADR 0035). */
export interface Reserved {
  id: string
  /** The Stock Unit's key. */
  unit: string
  batch: string
  expiry: string
  quantity: number
  expires: string
}

/** A dispense step the server refused, with its reason. */
export class DispenseFailed extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`dispense failed: ${status} ${reason}`)
    this.name = 'DispenseFailed'
  }
}

async function post(transport: Transport, path: string, body?: unknown, expected = 200): Promise<unknown> {
  const response = await transport.request({ method: 'POST', path, ...(body === undefined ? {} : { body }) })
  if (response.status !== expected) throw new DispenseFailed(response.status, ((response.body ?? {}) as { error?: string }).error ?? '')
  return response.body
}

/** What to prepare: from which store, which stock product, how much. */
export interface Preparing {
  store: string
  product: string
  quantity: number
}

/** Reserves stock for a dispense of `prescription`, under the client-generated `dispense` id. Nothing moves. */
export async function prepare(transport: Transport, prescription: string, dispense: string, preparing: Preparing): Promise<Reserved[]> {
  const body = (await post(transport, `/pharmacy/prescriptions/${prescription}/prepare`, { dispense, ...preparing }, 201)) as { reservations: Reserved[] }
  return body.reservations
}

/** Hands a prepared dispense over in full: only now does stock move. */
export async function handOver(transport: Transport, dispense: string): Promise<void> {
  await post(transport, `/pharmacy/dispenses/${dispense}/complete`)
}

/** Cancels a prepared dispense, freeing its reserved stock. */
export async function cancel(transport: Transport, dispense: string): Promise<void> {
  await post(transport, `/pharmacy/dispenses/${dispense}/cancel`)
}
