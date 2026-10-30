// Type-level tests: `tsc` passes only when every assignment below holds and every
// `@ts-expect-error` line is an error.

import { LosslessNumber } from 'lossless-json'

import type { Bundle, FhirResource, Observation, Patient, Questionnaire, ResourceType, ResourceTypeMap } from '../src'
import { resourceTypes } from '../src'

export const patient: Patient = {
  resourceType: 'Patient',
  id: 'p1',
  name: [{ family: 'Akello', given: ['Grace'], _given: [{ extension: [{ url: 'http://example.org/e', valueCode: 'CL' }] }] }],
  deceasedBoolean: false,
  birthDate: '1990-04-01',
  contact: [{ relationship: [{ text: 'mother' }], telecom: [{ system: 'phone', value: '0772 000000' }] }],
}

// Decimals and integers are LosslessNumber, as the client parses them (ADR 0052).
export const observation: Observation = {
  resourceType: 'Observation',
  status: 'final',
  code: { text: 'Body weight' },
  subject: { reference: 'Patient/p1' },
  valueQuantity: { value: new LosslessNumber('3.50'), unit: 'kg' },
  component: [{ code: { text: 'x' }, valueInteger: new LosslessNumber('3') }],
}

// contentReference: Questionnaire.item.item is a QuestionnaireItem.
export const questionnaire: Questionnaire = {
  resourceType: 'Questionnaire',
  status: 'draft',
  item: [{ linkId: '1', type: 'group', item: [{ linkId: '1.1', type: 'string' }] }],
}

export const bundle: Bundle = {
  resourceType: 'Bundle',
  type: 'collection',
  entry: [{ resource: patient }, { resource: observation }],
}

export function narrow(resource: FhirResource): string | undefined {
  return resource.resourceType === 'Patient' ? resource.name?.[0]?.family : undefined
}

export const kind: ResourceType = resourceTypes[0] ?? 'Patient'
export const mapped: ResourceTypeMap['Observation'] = observation

// @ts-expect-error: Observation.status (1..1) and code (1..1) are required.
export const missingRequired: Observation = { resourceType: 'Observation' }

// @ts-expect-error: a float is not a FHIR decimal; use LosslessNumber.
export const float: Observation = { ...observation, valueQuantity: { value: 3.5 } }

// @ts-expect-error: unknown elements are not allowed.
export const unknown: Patient = { resourceType: 'Patient', nickname: 'G' }

// @ts-expect-error: the resource type must match.
export const wrongType: Patient = { resourceType: 'Practitioner' }
