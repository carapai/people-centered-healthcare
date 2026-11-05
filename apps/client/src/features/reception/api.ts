import type { Bundle, Patient } from '@health/fhir-types'

import type { Transport } from '../../platform/types'

// Reception and queues (#102): `/api/reception`. The server checks permissions and every rule;
// the screens enforce nothing.

/** The service points a facility receives patients at. Facility configuration names them later. */
export const SERVICE_POINTS = ['triage', 'consultation', 'laboratory', 'pharmacy'] as const

/** A queue entry, as `/api/reception/queues/{servicePoint}` gives it. */
export interface QueueEntry {
  id: string
  encounter: string
  patient: string
  /** `{facility}/{service point}`. */
  servicePoint: string
  priority: 'routine' | 'urgent' | 'emergency'
  status: 'waiting' | 'called' | 'in-service' | 'done' | 'left'
  arrived: string
  called: [string, string] | null
}

/** A request the server refused, with its status and reason. */
export class ReceptionError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`reception request failed: ${status}`)
    this.name = 'ReceptionError'
  }
}

const reason = (body: unknown) => (typeof body === 'object' && body !== null && typeof (body as { error?: unknown }).error === 'string' ? (body as { error: string }).error : '')

async function call<T>(transport: Transport, method: 'GET' | 'POST', path: string, ok: number[], body?: unknown): Promise<T> {
  const response = await transport.request({
    method,
    path: `/reception${path}`,
    ...(body === undefined ? {} : { body }),
  })
  if (!ok.includes(response.status)) throw new ReceptionError(response.status, reason(response.body))
  return response.body as T
}

/** Checks `patient` (`Patient/{id}`) in at `servicePoint`, with a new Encounter id. */
export function checkIn(transport: Transport, id: string, patient: string, servicePoint: string, emergency: boolean) {
  return call(transport, 'POST', '/check-in', [201], {
    id,
    patient,
    servicePoint,
    class: emergency ? 'emergency' : 'ambulatory',
  })
}

export const queueKey = (servicePoint: string) => ['reception', 'queue', servicePoint] as const

export function queueQuery(transport: Transport, servicePoint: string) {
  return {
    queryKey: queueKey(servicePoint),
    queryFn: () => call<QueueEntry[]>(transport, 'GET', `/queues/${encodeURIComponent(servicePoint)}`, [200]),
  }
}

export type QueueStep = 'call' | 'serve' | 'recall' | 'finish' | 'leave'

export function step(transport: Transport, entry: string, next: QueueStep) {
  return call<QueueEntry>(transport, 'POST', `/queue/${encodeURIComponent(entry)}/${next}`, [200])
}

/** A patient's name as shown: given names, then family name. */
export function patientName(patient: Patient | undefined): string {
  const name = patient?.name?.[0]
  return [name?.given?.join(' '), name?.family].filter(Boolean).join(' ')
}

/** A patient's MRN at a facility (`urn:health:mrn/{facility}`), if they have one. */
export function mrn(patient: Patient): string | undefined {
  return patient.identifier?.find((i) => i.system?.startsWith('urn:health:mrn/') === true)?.value
}

/**
 * The facility patients of a search Bundle: those with an MRN. The client registry's golden
 * records match a search too, but nobody is checked in as one.
 */
export function patients(bundle: Bundle | undefined): Patient[] {
  return (bundle?.entry ?? []).flatMap((e) => (e.resource?.resourceType === 'Patient' && mrn(e.resource as Patient) !== undefined ? [e.resource as Patient] : []))
}
