import type { Questionnaire, QuestionnaireItem, ValueSet } from '@health/fhir-types'
import { QueryClient } from '@tanstack/react-query'
import { describe, expect, test } from 'vitest'

import { EXPANDED, boundCodesQuery, codesOf } from '../src/features/forms/answers'
import { FORM_LAYOUT_SCHEMA, type FormLayout, RADIO_MOST, widget } from '../src/features/forms/layout'
import { boundValueSets, choices } from '../src/features/forms/model'
import { fhirClient } from '../src/platform/fhir'
import { fakeTransport } from './fake'

// #843: forms in use offer a bound question's codes, expanded by the node the client talks to
// (central, or the facility's edge offline), and a large ValueSet is a drop-down.

const COMPLAINTS = 'https://health.go.ug/fhir/ValueSet/presenting-complaint'
const SYSTEM = 'https://health.go.ug/fhir/CodeSystem/presenting-complaint'

const expansion = (codes: number): ValueSet =>
  ({
    resourceType: 'ValueSet',
    url: COMPLAINTS,
    version: '1.0.0',
    status: 'active',
    expansion: {
      timestamp: '2026-11-24T08:00:00Z',
      total: codes,
      contains: Array.from({ length: codes }, (_, n) => ({ system: SYSTEM, version: '1.0.0', code: `c${n}`, display: `Complaint ${n}` })),
    },
  }) as unknown as ValueSet

describe('bound questions in forms in use', () => {
  test('the client expands a binding through the FHIR base, its active codes, a drop-down’s worth', async () => {
    const transport = fakeTransport(() => ({ body: expansion(3) }))
    const fhir = fhirClient(transport, '/api/fhir')
    const codes = await new QueryClient().fetchQuery(boundCodesQuery(fhir, `${COMPLAINTS}|1.0.0`))
    expect(transport.sent[0]).toMatchObject({
      method: 'GET',
      path: '/api/fhir/ValueSet/$expand',
      query: { url: COMPLAINTS, valueSetVersion: '1.0.0', activeOnly: 'true', count: String(EXPANDED) },
    })
    expect(codes).toEqual([
      { system: SYSTEM, version: '1.0.0', code: 'c0', display: 'Complaint 0' },
      { system: SYSTEM, version: '1.0.0', code: 'c1', display: 'Complaint 1' },
      { system: SYSTEM, version: '1.0.0', code: 'c2', display: 'Complaint 2' },
    ])
  })

  test('a binding without a version asks for none', async () => {
    const transport = fakeTransport(() => ({ body: expansion(1) }))
    await new QueryClient().fetchQuery(boundCodesQuery(fhirClient(transport), COMPLAINTS))
    expect(transport.sent[0]?.query).toEqual({ url: COMPLAINTS, activeOnly: 'true', count: String(EXPANDED) })
  })

  test('a binding the node cannot expand is an error, not an empty list', async () => {
    const transport = fakeTransport(() => ({ status: 404, body: { resourceType: 'OperationOutcome', issue: [{ severity: 'error', code: 'not-found' }] } }))
    await expect(new QueryClient().fetchQuery({ ...boundCodesQuery(fhirClient(transport), COMPLAINTS), retry: false })).rejects.toMatchObject({ status: 404 })
  })

  test('nested codes are offered and abstract groupers are not', () => {
    const valueSet = {
      resourceType: 'ValueSet',
      status: 'active',
      expansion: {
        timestamp: '2026-11-24T08:00:00Z',
        contains: [{ system: SYSTEM, code: 'resp', display: 'Respiratory', abstract: true, contains: [{ system: SYSTEM, code: 'cough', display: 'Cough' }] }, { system: SYSTEM, code: 'fever' }],
      },
    } as unknown as ValueSet
    expect(codesOf(valueSet)).toEqual([
      { system: SYSTEM, code: 'cough', display: 'Cough' },
      { system: SYSTEM, code: 'fever' },
    ])
  })

  test('every ValueSet a form binds, nested ones too, each once', () => {
    const form = {
      resourceType: 'Questionnaire',
      status: 'active',
      item: [
        { linkId: 'complaint', type: 'coding', answerValueSet: `${COMPLAINTS}|1.0.0` },
        { linkId: 'history', type: 'group', item: [{ linkId: 'past', type: 'coding', answerValueSet: `${COMPLAINTS}|1.0.0` }, { linkId: 'district', type: 'coding', answerValueSet: 'urn:districts|2.0.0' }] },
      ],
    } as unknown as Questionnaire
    expect(boundValueSets(form)).toEqual([`${COMPLAINTS}|1.0.0`, 'urn:districts|2.0.0'])
  })

  test('a bound question offers its codes, and a large ValueSet is a drop-down unless the layout says otherwise', () => {
    const item = { linkId: 'complaint', type: 'coding', answerValueSet: `${COMPLAINTS}|1.0.0` } as QuestionnaireItem
    const many = codesOf(expansion(40))
    const offered = choices(item, { [`${COMPLAINTS}|1.0.0`]: many })
    expect(offered).toHaveLength(40)
    expect(widget(item, undefined, offered.length)).toBe('select')
    expect(widget(item, undefined, RADIO_MOST)).toBe('radio')
    const layout: FormLayout = {
      $schema: FORM_LAYOUT_SCHEMA,
      url: 'urn:layout',
      version: '1.0.0',
      questionnaire: 'urn:form|1.0.0',
      sections: [],
      items: { complaint: { widget: 'radio' } },
    }
    expect(widget(item, layout, offered.length)).toBe('radio')
  })
})
