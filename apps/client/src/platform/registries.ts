import type { Bundle, OperationOutcome, Patient } from '@health/fhir-types'

import { FhirError } from './fhir'
import type { Transport } from './types'

/** What `Patient/$register` answered (patient-identity.md § "Did you mean" at registration). */
export type Registration =
  /** Possible matches: nothing was written. */
  | { kind: 'candidates'; bundle: Bundle }
  /** Too little to match on: search the registry by hand first. */
  | { kind: 'insufficient'; outcome: OperationOutcome }
  /** A new provisional source Patient. */
  | { kind: 'registered'; id: string; mrn: string; outcome: 'auto-linked' | 'queued' | 'new-identity' }

/** The Registries Bounded Context's commands (client-application.md § Ports). */
export interface RegistriesCommands {
  /** The national ID identifier system the registry matches on. */
  readonly ninSystem: string
  /** Registers `patient`; until `confirmedNew`, only answers with the candidates. */
  register(patient: Patient, confirmedNew: boolean, signal?: AbortSignal): Promise<Registration>
}

interface ParameterOut {
  name: string
  resource?: { resourceType?: string }
  valueString?: string
  valueCode?: string
  valueReference?: { reference?: string }
}

/** `base` is the FHIR base relative to the transport's base. */
export function registriesCommands(transport: Transport, base = '/fhir', ninSystem = 'urn:health:nin'): RegistriesCommands {
  return {
    ninSystem,
    async register(patient, confirmedNew, signal) {
      const response = await transport.request({
        method: 'POST',
        path: `${base}/Patient/$register`,
        headers: { accept: 'application/fhir+json', 'content-type': 'application/fhir+json' },
        body: {
          resourceType: 'Parameters',
          parameter: [
            { name: 'resource', resource: patient },
            { name: 'confirmedNew', valueBoolean: confirmedNew },
          ],
        },
        ...(signal === undefined ? {} : { signal }),
      })
      const body = response.body as { resourceType?: string; parameter?: ParameterOut[] } | undefined
      if ((response.status !== 200 && response.status !== 201) || body?.resourceType !== 'Parameters') {
        throw new FhirError(response.status, body?.resourceType === 'OperationOutcome' ? (body as unknown as OperationOutcome) : undefined)
      }
      const out = (name: string) => body.parameter?.find((p) => p.name === name)
      if (response.status === 200) {
        const candidates = out('candidates')?.resource
        if (candidates?.resourceType === 'OperationOutcome') return { kind: 'insufficient', outcome: candidates as OperationOutcome }
        return { kind: 'candidates', bundle: (candidates ?? { resourceType: 'Bundle', type: 'searchset' }) as Bundle }
      }
      const id = out('patient')?.valueReference?.reference?.replace(/^Patient\//, '') ?? ''
      const outcome = out('outcome')?.valueCode
      return {
        kind: 'registered',
        id,
        mrn: out('mrn')?.valueString ?? '',
        outcome: outcome === 'auto-linked' || outcome === 'queued' ? outcome : 'new-identity',
      }
    },
  }
}
