import type { ValueSet } from '@health/fhir-types'
import { describe, expect, test } from 'vitest'

import { addCode, bindingOf, changeCode, conceptActions, dropCode, excludeCode, includeCode, includeUnder, newValueSet, removeListed, removeRule, rules, suggestWidget, valueSetPath } from '../src/features/studio/terminology/model'

// The terminology browser's ValueSet editing (#523): what each action writes into the compose.
// The server expands the result; the acceptance criteria's expansions are tested there.

const SIGNS = 'https://health.go.ug/fhir/CodeSystem/anc-signs'
const ICD = 'http://id.who.int/icd/release/11/mms'

const blank: ValueSet = { resourceType: 'ValueSet', url: 'https://health.go.ug/fhir/ValueSet/danger-signs', version: '1.1.0-draft', status: 'draft', compose: { include: [] } }

describe('adding concepts', () => {
  test('a concept added alone joins the listing for its code system, once', () => {
    const one = includeCode(blank, SIGNS, 'bleeding', 'Vaginal bleeding')
    const two = includeCode(includeCode(one, SIGNS, 'fits'), SIGNS, 'fits')
    expect(two.compose?.include).toEqual([{ system: SIGNS, concept: [{ code: 'bleeding', display: 'Vaginal bleeding' }, { code: 'fits' }] }])
    const other = includeCode(two, ICD, '1A00')
    expect(other.compose?.include).toHaveLength(2)
  })

  test('a concept with everything under it is an is-a filter, and is no longer listed alone', () => {
    const listed = includeCode(blank, SIGNS, 'bleeding')
    const under = includeUnder(listed, SIGNS, 'bleeding')
    expect(under.compose?.include).toEqual([{ system: SIGNS, filter: [{ property: 'concept', op: 'is-a', value: 'bleeding' }] }])
    expect(includeUnder(under, SIGNS, 'bleeding')).toEqual(under)
    expect(rules(under)).toEqual([{ kind: 'include', index: 0, system: SIGNS, version: undefined, codes: [], under: 'bleeding', other: false }])
  })
})

describe('excluding concepts', () => {
  test('a listed code is taken out; nothing else selects it, so no exclude is written', () => {
    const vs = includeCode(includeCode(blank, SIGNS, 'bleeding'), SIGNS, 'fits')
    const out = excludeCode(vs, SIGNS, 'fits')
    expect(out.compose?.include).toEqual([{ system: SIGNS, concept: [{ code: 'bleeding' }] }])
    expect(out.compose?.exclude).toBeUndefined()
  })

  test('a code a filter still selects is excluded, and adding it again lifts the exclusion', () => {
    const vs = includeUnder(blank, SIGNS, 'danger')
    const out = excludeCode(vs, SIGNS, 'fits', 'Convulsions')
    expect(out.compose?.exclude).toEqual([{ system: SIGNS, concept: [{ code: 'fits', display: 'Convulsions' }] }])
    expect(rules(out).map((r) => r.kind)).toEqual(['include', 'exclude'])
    const back = includeCode(out, SIGNS, 'fits')
    expect(back.compose?.exclude).toBeUndefined()
  })

  test('excluding the concept an is-a rule names removes that rule', () => {
    const vs = includeUnder(blank, SIGNS, 'bleeding')
    expect(excludeCode(vs, SIGNS, 'bleeding').compose?.include).toEqual([])
  })
})

describe('removing rules and codes', () => {
  test('one listed code, then the rule once it lists nothing', () => {
    const vs = includeCode(includeCode(blank, SIGNS, 'bleeding'), SIGNS, 'fits')
    const one = removeListed(vs, 'include', 0, 'fits')
    expect(one.compose?.include).toEqual([{ system: SIGNS, concept: [{ code: 'bleeding' }] }])
    expect(removeListed(one, 'include', 0, 'bleeding').compose?.include).toEqual([])
    expect(removeRule(vs, 'include', 0).compose?.include).toEqual([])
  })

  test('a rule this browser does not edit is listed as other and kept', () => {
    const vs: ValueSet = { ...blank, compose: { include: [{ system: SIGNS, filter: [{ property: 'severity', op: '=', value: 'high' }] }] } }
    expect(rules(includeCode(vs, SIGNS, 'fits'))[0]).toMatchObject({ other: true, codes: [] })
    expect(includeCode(vs, SIGNS, 'fits').compose?.include[0]).toEqual(vs.compose?.include[0])
  })
})

describe('a new ValueSet and binding it', () => {
  test('its URL sits beside its sibling CodeSystem, and its file is named for its id', () => {
    const vs = newValueSet('danger signs', 'Danger signs', '1.1.0-draft', SIGNS)
    expect(vs).toMatchObject({ id: 'danger-signs', url: 'https://health.go.ug/fhir/ValueSet/danger-signs', name: 'DangerSigns', title: 'Danger signs', version: '1.1.0-draft', status: 'draft' })
    expect(valueSetPath(vs)).toBe('ValueSet-danger-signs.json')
  })

  test('the suggested widget: radio up to 7, a drop-down up to 30, search beyond', () => {
    expect([1, 7, 8, 30, 31].map(suggestWidget)).toEqual(['radio', 'radio', 'select', 'select', 'search'])
  })

  test('a binding records url|version', () => {
    expect(bindingOf(blank.url ?? '', '1.0.0')).toBe('https://health.go.ug/fhir/ValueSet/danger-signs|1.0.0')
    expect(bindingOf('urn:x', null)).toBe('urn:x')
  })
})

// Editing a local CodeSystem's concepts (#524): what the form sends and offers. The server keeps
// a published code fixed and works out the version; its tests cover the acceptance criteria.
describe('editing a local CodeSystem', () => {
  const path = 'CodeSystem-anc-signs.json'
  const fields = { code: ' spotting ', display: 'Spotting', definition: '  ', parent: 'bleeding', note: 'IMCI 2024 adds this sign' }

  test('a code added under another is sent with its parent and note, blanks left out', () => {
    expect(addCode(path, fields)).toEqual({ path, edit: 'add', code: 'spotting', display: 'Spotting', parent: 'bleeding', note: 'IMCI 2024 adds this sign' })
  })

  test('a change sends a new code only when it differs, and no parent for the top level', () => {
    expect(changeCode(path, 'spotting', { ...fields, parent: '', note: '' })).toEqual({ path, edit: 'change', code: 'spotting', display: 'Spotting' })
    expect(changeCode(path, 'spoting', { ...fields, definition: 'Light bleeding' })).toMatchObject({ code: 'spoting', newCode: 'spotting', definition: 'Light bleeding' })
    expect(dropCode(path, 'retire', 'fits', ' ')).toEqual({ path, edit: 'retire', code: 'fits' })
  })

  test('a published code is only retired, once; a new one is renamed, moved or deleted', () => {
    expect(conceptActions({ fixed: true, inactive: false })).toEqual({ rename: false, move: false, remove: false, retire: true })
    expect(conceptActions({ fixed: true, inactive: true }).retire).toBe(false)
    expect(conceptActions({ fixed: false, inactive: false })).toEqual({ rename: true, move: true, remove: true, retire: false })
  })
})
