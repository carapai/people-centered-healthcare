import type { Patient, QuestionnaireResponse } from '@health/fhir-types'

import { worklistKey } from '../../platform'
import type { Transport } from '../../platform/types'

/** A value as survivorship sees it: how far it is trusted, and which source said so. */
export interface Claim {
  value: string
  trust: 'nira-verified' | 'staff-verified' | 'staff-entered' | 'self-reported'
  recorded: string
  source: string
}

/** One pending registry review item (`/api/registry/reviews`). */
export type ReviewItem =
  | {
      id: string
      kind: 'potential-match'
      created_at: string
      overdue: boolean
      golden: string
      score: number
      source: Patient | null
      candidate: Patient | null
    }
  | {
      id: string
      kind: 'survivorship'
      created_at: string
      overdue: boolean
      golden: string
      field: string
      current: Claim
      incoming: Claim
    }
  | {
      id: string
      kind: 'registration-request'
      created_at: string
      overdue: boolean
      /** `Task/{id}`: the patient's Registration Request. */
      request: string
      /** What the patient submitted through the portal. */
      response: QuestionnaireResponse | null
      candidates: (Patient | null)[]
    }

export type Decision = 'same-person' | 'different-people' | 'keep-current' | 'take-incoming'

export const REVIEWS = 'registry-reviews'

/** A request the server refused, with its status. */
export class ReviewError extends Error {
  constructor(readonly status: number) {
    super(`registry review request failed: ${status}`)
    this.name = 'ReviewError'
  }
}

/** The pending queue, the oldest first. */
export function reviewQueue(transport: Transport) {
  return {
    queryKey: worklistKey(REVIEWS),
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<ReviewItem[]> => {
      const response = await transport.request({ method: 'GET', path: '/registry/reviews', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || !Array.isArray(response.body)) throw new ReviewError(response.status)
      return response.body as ReviewItem[]
    },
  }
}

/** The reviewer's decision on item `id`: carried out, recorded and audited by the server. */
export async function decide(transport: Transport, id: string, decision: Decision): Promise<void> {
  const response = await transport.request({ method: 'POST', path: `/registry/reviews/${encodeURIComponent(id)}`, body: { decision } })
  if (response.status !== 204) throw new ReviewError(response.status)
}

/** A reviewer's decision on a portal registration (#111): link the account to a candidate after
 * checking the holder's ID `document` in person, or create a new patient. */
export type RegistrationDecision = { decision: 'link-patient'; patient: string; document: string } | { decision: 'create-patient' }

/** The ID documents a reviewer may check in person. */
export const DOCUMENTS = ['national-id', 'passport', 'driving-licence', 'refugee-id'] as const

/** The reviewer's decision on registration `id`. */
export async function decideRegistration(transport: Transport, id: string, decision: RegistrationDecision): Promise<void> {
  const response = await transport.request({ method: 'POST', path: `/registry/reviews/${encodeURIComponent(id)}`, body: decision })
  if (response.status !== 204) throw new ReviewError(response.status)
}

/** The patient a portal registration describes, from its answers (`given`, `family`,
 * `birthDate`, `gender`), for comparing with the candidates. */
export function submitted(response: QuestionnaireResponse | null): Patient | null {
  if (response === null) return null
  const answer = (linkId: string) => {
    const value = response.item?.find((i) => i.linkId === linkId)?.answer?.[0]
    return value?.valueString ?? value?.valueDate ?? value?.valueCoding?.code
  }
  const given = answer('given')
  const family = answer('family')
  const birthDate = answer('birthDate')
  const gender = answer('gender')
  return {
    resourceType: 'Patient',
    name: [{ ...(family === undefined ? {} : { family }), ...(given === undefined ? {} : { given: [given] }) }],
    ...(birthDate === undefined ? {} : { birthDate }),
    ...(gender === 'male' || gender === 'female' || gender === 'other' || gender === 'unknown' ? { gender } : {}),
  }
}

/** One row of a side-by-side comparison. */
export interface Comparison {
  field: string
  source: string
  candidate: string
  agrees: boolean
}


function mrn(patient: Patient | null): string {
  return patient?.identifier?.find((i) => i.type?.coding?.some((c) => c.code === 'MR'))?.value ?? ''
}

/** The two records field by field, for a potential match; `ninSystem` names the national ID. */
export function compare(source: Patient | null, candidate: Patient | null, ninSystem: string): Comparison[] {
  const nin = (p: Patient | null) => p?.identifier?.find((i) => i.system === ninSystem)?.value ?? ''
  const fields: [string, (p: Patient | null) => string][] = [
    ['family', (p) => p?.name?.[0]?.family ?? ''],
    ['given', (p) => p?.name?.[0]?.given?.join(' ') ?? ''],
    ['birthDate', (p) => p?.birthDate ?? ''],
    ['gender', (p) => p?.gender ?? ''],
    ['nin', nin],
    ['phone', (p) => p?.telecom?.find((t) => t.system === 'phone')?.value ?? ''],
    ['district', (p) => p?.address?.[0]?.district ?? ''],
    ['mrn', mrn],
  ]
  return fields.map(([field, read]) => {
    const a = read(source)
    const b = read(candidate)
    return { field, source: a, candidate: b, agrees: a !== '' && a.toLowerCase() === b.toLowerCase() }
  })
}
