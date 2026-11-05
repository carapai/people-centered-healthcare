import type { Transport } from '../../platform/types'

// The consultation's command port (#104): `/api/consultation`. The node builds the FHIR
// resources from each command and checks permissions, the visit and every field; the UI never
// assembles a clinical resource.

/** A concept as the clinician names it; a code needs both its system and code. */
export interface Concept {
  display: string
  system?: string
  code?: string
}

export type Command =
  | { what: 'notes'; body: { id: string; text: string } }
  | { what: 'diagnoses'; body: { id: string; concept: Concept; provisional: boolean } }
  | { what: 'orders'; body: { id: string; kind: 'service'; concept: Concept; note?: string } }
  | { what: 'orders'; body: { id: string; kind: 'medication'; concept: Concept; dosage: string } }

/** A command the node refused, with its status and reason. */
export class ConsultationError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`consultation command failed: ${status}`)
    this.name = 'ConsultationError'
  }
}

/** Records `command` in the visit `encounter` (its id); 201 written, 200 already written. */
export async function record(transport: Transport, encounter: string, command: Command): Promise<void> {
  const response = await transport.request({
    method: 'POST',
    path: `/consultation/encounters/${encodeURIComponent(encounter)}/${command.what}`,
    body: command.body,
  })
  if (response.status !== 201 && response.status !== 200) {
    const body = response.body as { error?: unknown } | undefined
    throw new ConsultationError(response.status, typeof body?.error === 'string' ? body.error : '')
  }
}

/** Ends or moves the visit, through reception's Encounter routes. */
export async function dispose(transport: Transport, encounter: string, to: { complete: true } | { servicePoint: string }): Promise<void> {
  const path = `/reception/encounters/${encodeURIComponent(encounter)}/${'complete' in to ? 'status' : 'move'}`
  const body = 'complete' in to ? { status: 'completed' } : { servicePoint: to.servicePoint }
  const response = await transport.request({ method: 'POST', path, body })
  if (response.status !== 200) {
    const reason = response.body as { error?: unknown } | undefined
    throw new ConsultationError(response.status, typeof reason?.error === 'string' ? reason.error : '')
  }
}

export const summaryKey = (patient: string) => ['consultation', 'summary', patient] as const
