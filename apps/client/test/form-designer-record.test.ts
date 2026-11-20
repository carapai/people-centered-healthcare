import type { Questionnaire } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import mappingJson from '../../../packages/reference-packages/opd/Library-opd-triage-mapping.json'
import questionnaireJson from '../../../packages/reference-packages/opd/Questionnaire-opd-triage.json'
import {
  EXTRACT_CONTEXT,
  addObservation,
  blankMapping,
  contextLinkId,
  contextPath,
  isMappingOf,
  newMappingPath,
  recordProblems,
  renameInMapping,
  targets,
  withCode,
  withoutTarget,
  type Mapping,
} from '../src/features/studio/forms/record'

// *Becomes a record* in the form designer (#497): what an answer becomes, read from and written
// to the package's Extraction Mapping.

const questionnaire = questionnaireJson as unknown as Questionnaire
const mapping = mappingJson as unknown as Mapping

describe('reading the Extraction Mapping', () => {
  test('the OPD mapping is the triage form’s, and each question’s target is its template', () => {
    expect(isMappingOf(mapping, questionnaire.url)).toBe(true)
    expect(isMappingOf(mapping, 'https://example.org/other')).toBe(false)
    expect(targets(mapping, 'temperature')).toEqual([{ template: 'temperature', resourceType: 'Observation', code: { system: 'http://loinc.org', code: '8310-5', display: 'Body temperature' } }])
    expect(targets(mapping, 'triage-category')).toEqual([])
  })

  test('a context names the last linkId it walks to', () => {
    expect(contextLinkId({ extension: [{ url: EXTRACT_CONTEXT, valueString: "item.where(linkId = 'vitals').item.where(linkId = 'pulse' and answer.exists())" }] })).toBe('pulse')
    expect(contextLinkId({})).toBeUndefined()
  })
})

describe('editing what an answer becomes', () => {
  test('changing the code keeps the rest of the template', () => {
    const next = withCode(mapping, 'temperature', { system: 'http://loinc.org', code: '8331-1', display: 'Oral temperature' })
    expect(targets(next, 'temperature')[0]?.code).toEqual({ system: 'http://loinc.org', code: '8331-1', display: 'Oral temperature' })
    const before = mapping.contained?.find((t) => t.id === 'temperature')
    const after = next.contained?.find((t) => t.id === 'temperature')
    expect({ ...after, code: undefined }).toEqual({ ...before, code: undefined })
  })

  test('a coded question becomes an Observation whose value is the answer', () => {
    const at = (questionnaire.item ?? []).findIndex((item) => item.linkId === 'triage-category')
    const added = addObservation(mapping, questionnaire, [at], { system: 'http://loinc.org', code: '11283-9', display: 'Acuity assessment' })
    expect(added?.template).toBe('triage-category')
    const template = added?.mapping.contained?.find((t) => t.id === 'triage-category')
    expect(template).toMatchObject({
      resourceType: 'Observation',
      status: 'final',
      code: { coding: [{ system: 'http://loinc.org', code: '11283-9', display: 'Acuity assessment' }] },
      valueCodeableConcept: { coding: [{ extension: [{ valueString: 'answer.value' }] }] },
    })
    expect(contextLinkId(template ?? {})).toBe('triage-category')
    expect(added?.mapping.relatedArtifact?.at(-1)).toEqual({ type: 'composed-of', resourceReference: { reference: '#triage-category' } })
  })

  test('a nested question’s context walks down from its group', () => {
    expect(contextPath(questionnaire, [0, 0])).toBe("item.where(linkId = 'vitals').item.where(linkId = 'temperature' and answer.exists())")
  })

  test('a second record of the same question gets its own template id', () => {
    const added = addObservation(mapping, questionnaire, [0, 0], { code: 'x' })
    expect(added?.template).toBe('temperature-2')
    expect(targets(added?.mapping, 'temperature').map((t) => t.template)).toEqual(['temperature', 'temperature-2'])
  })

  test('a group cannot become an Observation', () => {
    expect(addObservation(mapping, questionnaire, [0], { code: 'x' })).toBeUndefined()
  })

  test('removing a record removes its template and the reference to it', () => {
    const next = withoutTarget(mapping, 'pulse')
    expect(targets(next, 'pulse')).toEqual([])
    expect(next.relatedArtifact?.some((r) => JSON.stringify(r).includes('#pulse'))).toBe(false)
  })

  test('renaming a new id carries the mapping along', () => {
    const renamed = renameInMapping(mapping, 'pulse', 'heart-rate')
    expect(targets(renamed, 'heart-rate').map((t) => t.template)).toEqual(['pulse'])
    expect(renameInMapping(mapping, 'nothing', 'else')).toBe(mapping)
  })
})

describe('a new mapping and its problems', () => {
  test('a form with no mapping gets one for its major version', () => {
    const blank = blankMapping({ ...questionnaire, version: '2.3.0-draft' })
    expect(isMappingOf(blank, questionnaire.url)).toBe(true)
    expect(blank).toMatchObject({ url: 'https://health.go.ug/fhir/Library/opd-triage-mapping', version: '2.3.0-draft' })
    expect(JSON.stringify(blank)).toContain('>=2.0.0 <3.0.0')
    expect(newMappingPath('Questionnaire-opd-triage.json')).toBe('Library-opd-triage-mapping.json')
  })

  test('a record with no code, or of a question no longer in the form, is a problem', () => {
    const noCode = withCode(mapping, 'pulse', { code: '' })
    expect(recordProblems(noCode, questionnaire)).toEqual([{ code: 'record-no-code', linkId: 'pulse', template: 'pulse' }])
    const gone = { ...questionnaire, item: [] }
    expect(recordProblems(mapping, gone).some((p) => p.code === 'record-orphan' && p.linkId === 'temperature')).toBe(true)
  })
})
