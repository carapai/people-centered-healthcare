import type { Transport } from '../../platform/types'

/** One line of a goods received note. */
export interface ReceivedLine {
  product: string
  batch: string
  /** `YYYY-MM-DD`. */
  expiry: string
  quantity: number
  pack_size: number
  /** A decimal, e.g. `1.50`. */
  unit_cost: string
}

/** A goods received note against a supplier delivery (#130). */
export interface GoodsReceived {
  store: string
  supplier: string
  delivery: string
  lines: ReceivedLine[]
}

/** A Stock Unit of a store, as `/api/stock/on-hand` reports it. */
export interface OnHand {
  product: string
  batch: string
  expiry: string
  funding_source: string
  status: 'usable' | 'quarantine' | 'restricted'
  on_hand: number
  available: number
  flagged: boolean
  unit_cost: string | null
}

/** A reorder suggestion. */
export interface Suggestion {
  product: string
  on_hand: number
  monthly: string
  min: number
  max: number
  order: number
}

/** What a GS1 DataMatrix holds. */
export interface Gs1 {
  gtin: string | null
  expiry: string | null
  batch: string | null
  serial: string | null
}

/** A request the server refused, with its status and reason. */
export class StockError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`stock request failed: ${status}`)
    this.name = 'StockError'
  }
}

const reason = (body: unknown) => (typeof body === 'object' && body !== null && typeof (body as { error?: unknown }).error === 'string' ? (body as { error: string }).error : '')

export const onHandKey = (store: string) => ['stock', 'on-hand', store] as const
export const reorderKey = (store: string) => ['stock', 'reorder', store] as const

/** A store's stock on hand. */
export function onHandQuery(transport: Transport, store: string) {
  return {
    queryKey: onHandKey(store),
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<OnHand[]> => {
      const response = await transport.request({ method: 'GET', path: `/stock/on-hand?store=${encodeURIComponent(store)}`, ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || !Array.isArray(response.body)) throw new StockError(response.status, reason(response.body))
      return response.body as OnHand[]
    },
  }
}

/** A store's reorder suggestions from the last 30 days. */
export function reorderQuery(transport: Transport, store: string) {
  return {
    queryKey: reorderKey(store),
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Suggestion[]> => {
      const response = await transport.request({ method: 'GET', path: `/stock/reorder?store=${encodeURIComponent(store)}&days=30`, ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || !Array.isArray(response.body)) throw new StockError(response.status, reason(response.body))
      return response.body as Suggestion[]
    },
  }
}

/** Receives a goods received note: every line a receipt at its unit cost. */
export async function receiveGoods(transport: Transport, note: GoodsReceived): Promise<string> {
  const response = await transport.request({ method: 'POST', path: '/stock/goods-received', body: note })
  if (response.status !== 201) throw new StockError(response.status, reason(response.body))
  return (response.body as { id: string }).id
}

/** Reads a scanned or printed GS1 DataMatrix. */
export async function readGs1(transport: Transport, data: string): Promise<Gs1> {
  const response = await transport.request({ method: 'POST', path: '/stock/gs1', body: { data } })
  if (response.status !== 200) throw new StockError(response.status, reason(response.body))
  return response.body as Gs1
}

/** Whether a line is complete enough to send. */
export function lineComplete(line: ReceivedLine): boolean {
  return (
    line.product.trim() !== '' &&
    line.batch.trim() !== '' &&
    /^\d{4}-\d{2}-\d{2}$/.test(line.expiry) &&
    line.quantity > 0 &&
    line.pack_size > 0 &&
    /^\d+(\.\d+)?$/.test(line.unit_cost.trim())
  )
}

/** A ward's requisition (SupplyRequest), as `/api/stock/requisitions` reports it. */
export interface Requisition {
  id: string
  requesting_store: string
  supplying_store: string
  lines: { product: string; quantity: number }[]
  status: 'requested' | 'approved' | 'fulfilled' | 'rejected'
  requested_by: string
  decided_by: string | null
  transfer: string | null
}

/** One batch picked to fulfil a requisition. */
export interface Picked {
  product: string
  batch: string
  expiry: string
  funding_source: string
  quantity: number
}

export const requisitionsKey = (store: string) => ['stock', 'requisitions', store] as const

/** The requisitions a store asked for or was asked, newest first. */
export function requisitionsQuery(transport: Transport, store: string) {
  return {
    queryKey: requisitionsKey(store),
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Requisition[]> => {
      const response = await transport.request({ method: 'GET', path: `/stock/requisitions?store=${encodeURIComponent(store)}`, ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || !Array.isArray(response.body)) throw new StockError(response.status, reason(response.body))
      return response.body as Requisition[]
    },
  }
}

/** A ward asks another store for products. */
export async function requestStock(transport: Transport, requesting: string, supplying: string, lines: { product: string; quantity: number }[]): Promise<string> {
  const response = await transport.request({
    method: 'POST',
    path: '/stock/requisitions',
    body: { requesting_store: requesting, supplying_store: supplying, lines },
  })
  if (response.status !== 201) throw new StockError(response.status, reason(response.body))
  return (response.body as { id: string }).id
}

/** The supplying store approves or refuses requisition `id`. */
export async function decideRequisition(transport: Transport, id: string, approve: boolean): Promise<Requisition> {
  const response = await transport.request({ method: 'POST', path: `/stock/requisitions/${encodeURIComponent(id)}/decision`, body: { approve } })
  if (response.status !== 200) throw new StockError(response.status, reason(response.body))
  return response.body as Requisition
}

/** Fulfils requisition `id` with the batches picked: they leave as a transfer to the ward. */
export async function fulfilRequisition(transport: Transport, id: string, lines: Picked[]): Promise<Requisition> {
  const response = await transport.request({ method: 'POST', path: `/stock/requisitions/${encodeURIComponent(id)}/fulfil`, body: { lines } })
  if (response.status !== 200) throw new StockError(response.status, reason(response.body))
  return response.body as Requisition
}
