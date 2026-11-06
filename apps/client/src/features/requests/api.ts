import type { Transport } from '../../platform/types'

/** A patient's request (#114), as the portal and staff routes give it. */
export interface PatientRequest {
  /** The proposal, `Type/id`. */
  reference: string
  kind: 'refill' | 'care' | 'appointment'
  state: 'pending' | 'accepted' | 'closed'
  title: string
  patient: string
  facility: string
  /** Why it was declined or withdrawn. */
  reason: string | null
  /** A booked appointment's start. */
  start: string | null
}

/** A prescription the patient may ask a refill of. */
export interface Refillable {
  id: string
  medicine: { text: string }
  dosage: string
}

/** What the patient asks for. */
export type Ask =
  | { kind: 'refill'; prior: string }
  | { kind: 'care'; facility: string; what: string }
  | { kind: 'appointment'; facility: string; reason: string; preferred?: string }

/** A request the server refused: its status, reason and any safety warnings to acknowledge. */
export class RequestFailed extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
    readonly warnings: string[] = [],
  ) {
    super(`request failed: ${status} ${reason}`)
    this.name = 'RequestFailed'
  }
}

export const MY_REQUESTS = ['portal', 'requests'] as const
export const MY_PRESCRIPTIONS = ['portal', 'prescriptions'] as const

function failure(status: number, body: unknown): RequestFailed {
  const answer = (body ?? {}) as { error?: string; warnings?: unknown[] }
  const warnings = (answer.warnings ?? []).map((w) => (typeof w === 'string' ? w : JSON.stringify(w)))
  return new RequestFailed(status, answer.error ?? '', warnings)
}

async function get<T>(transport: Transport, path: string, signal?: AbortSignal): Promise<T> {
  const response = await transport.request({ method: 'GET', path, ...(signal === undefined ? {} : { signal }) })
  if (response.status !== 200) throw failure(response.status, response.body)
  return response.body as T
}

/** The signed-in patient's requests. */
export function myRequestsQuery(transport: Transport) {
  return { queryKey: MY_REQUESTS, queryFn: ({ signal }: { signal?: AbortSignal }) => get<PatientRequest[]>(transport, '/portal/requests', signal) }
}

/** The prescriptions the signed-in patient may ask a refill of (with full access only). */
export function myPrescriptionsQuery(transport: Transport) {
  return { queryKey: MY_PRESCRIPTIONS, queryFn: ({ signal }: { signal?: AbortSignal }) => get<Refillable[]>(transport, '/portal/prescriptions', signal) }
}

/** The reference a refill names. */
export const prescriptionReference = (id: string) => (id.startsWith('MedicationRequest/') ? id : `MedicationRequest/${id}`)

/** Sends a request for `patient`, under the client-generated `id`. */
export async function ask(transport: Transport, id: string, patient: string, asked: Ask, note: string): Promise<PatientRequest> {
  const body = { id, patient, ...asked, ...(note.trim() === '' ? {} : { note: note.trim() }) }
  const response = await transport.request({ method: 'POST', path: '/portal/requests', body })
  if (response.status !== 201) throw failure(response.status, response.body)
  return response.body as PatientRequest
}

/** Withdraws one of the patient's pending requests. */
export async function withdraw(transport: Transport, reference: string): Promise<PatientRequest> {
  const response = await transport.request({ method: 'POST', path: `/portal/requests/${reference}/withdraw` })
  if (response.status !== 200) throw failure(response.status, response.body)
  return response.body as PatientRequest
}

/** What staff send to accept: an appointment's time, or acknowledged safety warnings. */
export interface Acceptance {
  acknowledge?: boolean
  start?: string
  end?: string
}

/** Staff accept a pending request, writing its order or booking. */
export async function accept(transport: Transport, reference: string, acceptance: Acceptance): Promise<PatientRequest> {
  const response = await transport.request({ method: 'POST', path: `/requests/${reference}/accept`, body: acceptance })
  if (response.status !== 200) throw failure(response.status, response.body)
  return response.body as PatientRequest
}

/** Staff decline a pending request, with a reason. */
export async function decline(transport: Transport, reference: string, reason: string): Promise<PatientRequest> {
  const response = await transport.request({ method: 'POST', path: `/requests/${reference}/decline`, body: { reason } })
  if (response.status !== 200) throw failure(response.status, response.body)
  return response.body as PatientRequest
}

/** Whether `resource` is a patient's proposal still waiting for staff. */
export function isPendingProposal(resource: Record<string, unknown>): boolean {
  if (resource['resourceType'] === 'Appointment') return resource['status'] === 'proposed'
  return resource['intent'] === 'proposal' && resource['status'] === 'active'
}
