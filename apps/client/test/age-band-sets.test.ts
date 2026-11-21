import { createIntl } from 'react-intl'
import { describe, expect, test } from 'vitest'

import ancBands from '../../../packages/reference-packages/core/Library-age-bands-anc.json'
import { editorOf, newDesign, sections } from '../src/features/studio/designs'
import { bandLabel, bandProblems, flagged, isAgeBandSet, nextVersion, readAgeBands, writeAgeBands, type AgeBandTable } from '../src/features/studio/indicators/ageBands'
import { ageBandMessages as m, designMessages as dm } from '../src/i18n/messages'

// #551: an Age Band Set is edited as a small table (name, unit, the date age is taken at, from–to
// rows), with gaps and overlaps flagged as the author types. A band change takes the next major
// version, a rename the next patch.

const read = (text: string) => {
  const result = readAgeBands(text)
  if ('refusal' in result) throw new Error(result.refusal)
  return result
}

const moh = () => read(JSON.stringify(ancBands))

describe('reading and writing', () => {
  test('the MoH ANC set reads as its table', () => {
    const { table } = moh()
    expect(table).toEqual({
      title: 'MoH ANC age bands',
      unit: 'years',
      ageAt: 'period-end',
      bands: [
        { from: 10, to: 14 },
        { from: 15, to: 19 },
        { from: 20, to: 24 },
        { from: 25, to: null },
      ],
    })
    expect(table.bands.map((b) => bandLabel(b, table.unit))).toEqual(['10–14 years', '15–19 years', '20–24 years', '25+ years'])
    expect(bandLabel({ from: 1, to: 1 }, 'years')).toBe('1 year')
  })

  test('written back, it keeps everything it does not edit and reads the same', () => {
    const { library, table } = moh()
    const edited: AgeBandTable = { ...table, bands: [...table.bands.slice(0, 3), { from: 25, to: 49 }] }
    const written = writeAgeBands(library, edited, '2.0.0')
    expect(written['url']).toBe(ancBands.url)
    expect(written['description']).toBe(ancBands.description)
    expect(written['version']).toBe('2.0.0')
    expect(read(JSON.stringify(written)).table).toEqual(edited)
  })

  test('another Library is not an Age Band Set', () => {
    expect(readAgeBands(JSON.stringify({ resourceType: 'Library', type: { coding: [{ code: 'workflow-definition' }] } }))).toEqual({ refusal: 'not-age-band-set' })
    expect(readAgeBands('{')).toEqual({ refusal: 'not-json' })
  })
})

const table = (bands: AgeBandTable['bands']): AgeBandTable => ({ title: 'Bands', unit: 'years', ageAt: 'period-end', bands })

describe('gaps and overlaps', () => {
  test('contiguous bands have no problems, in any order', () => {
    expect(bandProblems(moh().table)).toEqual([])
    expect(
      bandProblems(
        table([
          { from: 15, to: 19 },
          { from: 10, to: 14 },
        ]),
      ),
    ).toEqual([])
  })

  test('a gap is flagged on both rows with the ages it leaves out', () => {
    const problems = bandProblems(
      table([
        { from: 10, to: 14 },
        { from: 20, to: null },
      ]),
    )
    expect(problems).toEqual([{ kind: 'gap', rows: [0, 1], from: 15, to: 19 }])
    expect(flagged(problems)).toEqual(new Set([0, 1]))
    const intl = createIntl({ locale: 'en', messages: {} })
    expect(intl.formatMessage(m.abGap, { first: '10–14 years', second: '20+ years', ages: intl.formatMessage(m.abAges, { from: 15, to: 19, unit: 'years' }) })).toBe(
      'Gap between 10–14 years and 20+ years: 15–19 years are in no band.',
    )
  })

  test('an overlap, and an open band followed by another, are flagged', () => {
    expect(
      bandProblems(
        table([
          { from: 10, to: 15 },
          { from: 15, to: 19 },
        ]),
      ),
    ).toEqual([{ kind: 'overlap', rows: [0, 1], from: 15, to: 15 }])
    expect(
      bandProblems(
        table([
          { from: 20, to: null },
          { from: 25, to: 29 },
        ]),
      ),
    ).toEqual([{ kind: 'overlap', rows: [0, 1], from: 25, to: 29 }])
  })

  test('a row without From, below zero or going down is its own problem', () => {
    expect(
      bandProblems({
        ...table([
          { from: null, to: 4 },
          { from: -1, to: 4 },
          { from: 9, to: 5 },
        ]),
        title: ' ',
      }).map((p) => p.kind),
    ).toEqual(['no-title', 'no-from', 'negative', 'goes-down'])
    expect(bandProblems(table([])).map((p) => p.kind)).toEqual(['no-bands'])
  })
})

describe('the version', () => {
  const published = { version: '1.0.0', table: moh().table }

  test('a band change is the next major version, a rename the next patch', () => {
    const moved = { ...published.table, bands: [{ from: 10, to: 15 }, ...published.table.bands.slice(1)] }
    expect(nextVersion(published, '1.0.0', moved)).toEqual({ version: '2.0.0', reason: 'bands' })
    expect(nextVersion(published, '1.0.0', { ...published.table, unit: 'months' })).toEqual({ version: '2.0.0', reason: 'bands' })
    expect(nextVersion(published, '1.0.0', { ...published.table, title: 'MoH antenatal age bands' })).toEqual({ version: '1.0.1', reason: 'renamed' })
    expect(nextVersion(published, '1.0.0', published.table)).toEqual({ version: '1.0.0', reason: 'unchanged' })
    expect(nextVersion(null, '0.1.0', moved)).toEqual({ version: '0.1.0', reason: 'new' })
  })
})

describe('in the Studio', () => {
  test('Age band sets is a section of its own, created with New and opened in its table', () => {
    expect(sections.find((s) => s.id === 'age-bands')).toEqual({ id: 'age-bands', kinds: ['age-band-set'], creates: true })
    expect(editorOf({ kind: 'age-band-set', path: 'Library-age-bands-anc.json', table: null }, 'ug.moh.core@1.0.0')).toEqual({
      to: '/studio/$id/age-bands/$',
      params: { id: 'ug.moh.core@1.0.0', _splat: 'Library-age-bands-anc.json' },
    })
    const made = newDesign('age-band-set', 'HIV age bands', 'https://health.go.ug/fhir', '0.2.0', [])
    expect(made.path).toBe('Library-hiv-age-bands.json')
    expect(isAgeBandSet(made.resource)).toBe(true)
    expect(made.resource['url']).toBe('https://health.go.ug/fhir/Library/hiv-age-bands')
    expect(read(JSON.stringify(made.resource)).table).toEqual({ title: 'HIV age bands', unit: 'years', ageAt: 'period-end', bands: [{ from: 0, to: null }] })
    const intl = createIntl({ locale: 'en', messages: {} })
    expect(intl.formatMessage(dm.dhSection, { section: 'ageBands' })).toBe('Age band sets')
    expect(intl.formatMessage(dm.dhKind, { kind: 'ageBandSet' })).toBe('Age band set')
  })
})
