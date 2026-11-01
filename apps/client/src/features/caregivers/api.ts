import type { Transport } from '../../platform/types'

/** What a caregiver may do (the Consent's scope). */
export const SCOPES = ['view-records', 'book-appointments', 'request-refills', 'receive-messages'] as const
export type CaregiverScope = (typeof SCOPES)[number]

/** A caregiver grant, as `/api/caregivers` reports it (#113). */
export interface Grant {
  id: string
  caregiver: string
  patient: string
  authority: 'guardian' | 'delegated'
  scopes: CaregiverScope[]
  expires: string
  /** The document checked, `delivery`, or null. */
  verified: string | null
  adolescent_consents: boolean
  status: 'active' | 'revoked' | 'expired' | 'unverified'
}

/** A dependent, as `/api/portal/dependents` reports it to the caregiver. */
export interface Dependent extends Grant {
  name: string | null
  /** A guardian without the adolescent's consent: their sensitive records stay with them. */
  confidential_withheld: boolean
}

/** A caregiver for staff to record. */
export interface NewCaregiver {
  patient: string
  phone: string
  relationship: string
  authority: 'guardian' | 'delegated'
  scopes: CaregiverScope[]
  /** The last day of access (`YYYY-MM-DD`). */
  expires: string
  document: string
}

/** A request the server refused, with its status and message. */
export class CaregiverError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`caregiver request failed: ${status}`)
    this.name = 'CaregiverError'
  }
}

const reason = (body: unknown) => (typeof body === 'object' && body !== null && typeof (body as { error?: unknown }).error === 'string' ? (body as { error: string }).error : '')

export const caregiversKey = (patient: string) => ['caregivers', patient] as const

/** A patient's caregivers. */
export function caregiversQuery(transport: Transport, patient: string) {
  return {
    queryKey: caregiversKey(patient),
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Grant[]> => {
      const response = await transport.request({
        method: 'GET',
        path: `/caregivers?patient=${encodeURIComponent(patient)}`,
        ...(signal === undefined ? {} : { signal }),
      })
      if (response.status !== 200 || !Array.isArray(response.body)) throw new CaregiverError(response.status, reason(response.body))
      return response.body as Grant[]
    },
  }
}

/** Records a caregiver, verified against the document checked. */
export async function recordCaregiver(transport: Transport, caregiver: NewCaregiver): Promise<Grant> {
  const response = await transport.request({ method: 'POST', path: '/caregivers', body: caregiver })
  if (response.status !== 201) throw new CaregiverError(response.status, reason(response.body))
  return response.body as Grant
}

/** Revokes grant `id`. */
export async function revokeCaregiver(transport: Transport, id: string): Promise<Grant> {
  const response = await transport.request({ method: 'POST', path: `/caregivers/${encodeURIComponent(id)}/revoke` })
  if (response.status !== 200) throw new CaregiverError(response.status, reason(response.body))
  return response.body as Grant
}

/** Records the adolescent's consent on grant `id`, given or withdrawn. */
export async function adolescentConsent(transport: Transport, id: string, consents: boolean): Promise<Grant> {
  const response = await transport.request({ method: 'POST', path: `/caregivers/${encodeURIComponent(id)}/adolescent-consent`, body: { consents } })
  if (response.status !== 200) throw new CaregiverError(response.status, reason(response.body))
  return response.body as Grant
}

export const DEPENDENTS = ['portal', 'dependents'] as const

/** The people the signed-in caregiver cares for. */
export function dependentsQuery(transport: Transport) {
  return {
    queryKey: DEPENDENTS,
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Dependent[]> => {
      const response = await transport.request({ method: 'GET', path: '/portal/dependents', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || !Array.isArray(response.body)) throw new CaregiverError(response.status, reason(response.body))
      return response.body as Dependent[]
    },
  }
}
