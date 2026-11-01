import type { QuestionnaireResponse } from '@health/fhir-types'

import type { Transport } from '../../platform/types'

/** The self-registration Questionnaire (patient-identity.md § Self-registration; #111). */
export const QUESTIONNAIRE = 'urn:health:questionnaire/self-registration'

/** A patient's registration, as `/api/portal/registration` reports it. */
export interface Registration {
  /** The §59 state: `review` waits for staff; `completed` is linked or created. */
  status: 'draft' | 'submitted' | 'verified' | 'review' | 'matched' | 'completed'
  /** The Patient the account is linked to, once there is one. */
  patient: string | null
  /** What the account may see: below level3, only what the patient submitted. */
  access: 'none' | 'self-submitted' | 'records'
}

/** What the patient enters. */
export interface Answers {
  given: string
  family: string
  birthDate: string
  gender: '' | 'female' | 'male' | 'other' | 'unknown'
}

export const REGISTRATION = ['portal', 'registration'] as const

/** A request the server refused, with its status. */
export class PortalError extends Error {
  constructor(readonly status: number) {
    super(`portal request failed: ${status}`)
    this.name = 'PortalError'
  }
}

/** The signed-in patient's registration; `null` before they submit one. */
export function registrationQuery(transport: Transport) {
  return {
    queryKey: REGISTRATION,
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Registration | null> => {
      const response = await transport.request({ method: 'GET', path: '/portal/registration', ...(signal === undefined ? {} : { signal }) })
      if (response.status === 404) return null
      if (response.status !== 200) throw new PortalError(response.status)
      return response.body as Registration
    },
  }
}

/** Whether the answers are complete enough to send. */
export function canSubmit(answers: Answers): boolean {
  return answers.given.trim() !== '' && answers.family.trim() !== '' && answers.birthDate !== '' && answers.gender !== ''
}

/** The answers as the self-registration QuestionnaireResponse. */
export function questionnaireResponse(answers: Answers): QuestionnaireResponse {
  return {
    resourceType: 'QuestionnaireResponse',
    questionnaire: QUESTIONNAIRE,
    status: 'completed',
    item: [
      { linkId: 'given', answer: [{ valueString: answers.given.trim() }] },
      { linkId: 'family', answer: [{ valueString: answers.family.trim() }] },
      { linkId: 'birthDate', answer: [{ valueDate: answers.birthDate }] },
      { linkId: 'gender', answer: [{ valueCoding: { code: answers.gender } }] },
    ],
  }
}

/** Submits the patient's registration. */
export async function submitRegistration(transport: Transport, answers: Answers): Promise<Registration> {
  const sent = await transport.request({ method: 'POST', path: '/portal/registration', body: questionnaireResponse(answers) })
  if (sent.status !== 201) throw new PortalError(sent.status)
  return sent.body as Registration
}
