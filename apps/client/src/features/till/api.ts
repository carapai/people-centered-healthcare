import type { Transport } from '../../platform/types'

/** A cashier shift, as `/api/payments` reports it (#254). */
export interface Shift {
  id: string
  till: string
  cashier: string
  status: 'open' | 'awaiting-approval' | 'settled'
  float: string
  expected: string
  counted: string | null
  variance: string | null
  approved_by: string | null
}

/** The payment methods. */
export const METHODS = ['cash', 'mtn-momo', 'airtel-money', 'card', 'bank-transfer', 'customer-reported'] as const
export type PaymentMethod = (typeof METHODS)[number]

/** A payment and its receipt. */
export interface Payment {
  id: string
  method: PaymentMethod
  deposit: boolean
  amount: string
  payer: string
  reference: string | null
  status: 'pending' | 'pending-unverified' | 'posted' | 'voided' | 'expired' | 'reversed'
  /** "provisional" until provider evidence (ADR 0032; §68). */
  receipt: { number: string | null; status: 'provisional' | 'confirmed' | 'void' }
  refunded: string
}

/** A request the server refused, with its status and reason. */
export class TillError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`till request failed: ${status}`)
    this.name = 'TillError'
  }
}

const reason = (body: unknown) => (typeof body === 'object' && body !== null && typeof (body as { error?: unknown }).error === 'string' ? (body as { error: string }).error : '')

async function call<T>(transport: Transport, method: 'GET' | 'POST', path: string, ok: number, body?: unknown): Promise<T> {
  const response = await transport.request({ method, path: `/payments${path}`, ...(body === undefined ? {} : { body }) })
  if (response.status !== ok) throw new TillError(response.status, reason(response.body))
  return response.body as T
}

export const shiftKey = (till: string) => ['till', till] as const

/** The open shift of `till` with its payments, or `null` when the till is closed. */
export function openShiftQuery(transport: Transport, till: string) {
  return {
    queryKey: shiftKey(till),
    queryFn: async (): Promise<{ shift: Shift; payments: Payment[] } | null> => {
      const response = await transport.request({ method: 'GET', path: `/payments/shifts/open?till=${encodeURIComponent(till)}` })
      if (response.status === 404) return null
      if (response.status !== 200) throw new TillError(response.status, reason(response.body))
      const shift = response.body as Shift
      return call(transport, 'GET', `/shifts/${encodeURIComponent(shift.id)}`, 200)
    },
  }
}

export const openShift = (transport: Transport, till: string, float: string) => call<Shift>(transport, 'POST', '/shifts', 201, { till, float })
export const closeShift = (transport: Transport, id: string, counted: string) =>
  call<Shift>(transport, 'POST', `/shifts/${encodeURIComponent(id)}/close`, 200, { counted })
export const approveShift = (transport: Transport, id: string) => call<Shift>(transport, 'POST', `/shifts/${encodeURIComponent(id)}/approve`, 200)
export const shiftById = (transport: Transport, id: string) => call<{ shift: Shift; payments: Payment[] }>(transport, 'GET', `/shifts/${encodeURIComponent(id)}`, 200)

/** Takes a payment: cash is final; any other method is pending with its provider reference. */
export function takePayment(transport: Transport, shift: string, method: PaymentMethod, amount: string, payer: string, reference: string): Promise<Payment> {
  return method === 'cash'
    ? call(transport, 'POST', '/cash', 201, { shift, amount, payer })
    : call(transport, 'POST', '/pending', 201, { shift, method, amount, payer, reference })
}

/** A supervisor enters a provider statement line: the pending payments it matches are confirmed. */
export const reconcile = (transport: Transport, reference: string, amount: string) =>
  call<{ confirmed: string[]; mismatched: unknown[]; unknown: string[] }>(transport, 'POST', '/statements', 200, { lines: [{ reference, amount }] })
