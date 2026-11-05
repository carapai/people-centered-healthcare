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

// Clinical notes (#107): `/api/notes`. Signed notes are never edited; they are amended.

export type NoteKind = 'progress' | 'consultation' | 'admission' | 'procedure' | 'discharge' | 'letter'

/** Each kind's sections, as the node starts them (`health-clinical-documentation`). */
export const NOTE_SECTIONS: Record<NoteKind, string[]> = {
  progress: ['Subjective', 'Objective', 'Assessment', 'Plan'],
  consultation: ['Note'],
  admission: ['History', 'Examination', 'Assessment', 'Plan'],
  procedure: ['Note'],
  discharge: ['Course', 'Condition at discharge', 'Follow-up'],
  letter: ['Note'],
}

export interface NoteSection {
  title: string
  text: string
}

export interface ClinicalNote {
  id: string
  kind: NoteKind
  patient: string
  author: string
  date: string
  status: 'preliminary' | 'final' | 'amended' | 'appended' | 'corrected' | 'cancelled' | 'entered-in-error'
  sections: NoteSection[]
  signed: { by: string; at: string } | null
  relatesTo: { relation: 'replaces' | 'appends'; note: string } | null
}

async function notesCall<T>(transport: Transport, method: 'GET' | 'POST', path: string, body?: unknown): Promise<T> {
  const response = await transport.request({ method, path: `/notes${path}`, ...(body === undefined ? {} : { body }) })
  if (response.status !== 200 && response.status !== 201) {
    const reason = response.body as { error?: unknown } | undefined
    throw new ConsultationError(response.status, typeof reason?.error === 'string' ? reason.error : '')
  }
  return response.body as T
}

export const notesKey = (patient: string) => ['consultation', 'notes', patient] as const

export const listNotes = (transport: Transport, patient: string) => notesCall<ClinicalNote[]>(transport, 'GET', `/patients/${encodeURIComponent(patient)}/notes`)

export const draftNote = (transport: Transport, note: { id: string; kind: NoteKind; patient: string; encounter: string; sections: NoteSection[] }) =>
  notesCall<ClinicalNote>(transport, 'POST', '/notes', note)

export const signNote = (transport: Transport, id: string) => notesCall<ClinicalNote>(transport, 'POST', `/notes/${encodeURIComponent(id)}/sign`)

export const amendNote = (transport: Transport, original: string, amendment: { id: string; relation: 'replaces' | 'appends'; sections: NoteSection[] }) =>
  notesCall<ClinicalNote>(transport, 'POST', `/notes/${encodeURIComponent(original)}/amend`, amendment)
