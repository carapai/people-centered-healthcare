import { createIntl } from 'react-intl'
import { describe, expect, test } from 'vitest'

import population from '../../../packages/reference-packages/population/configuration/population.json'
import { newDesign } from '../src/features/studio/designs'
import { ESTIMATES_FILE, POPULATION_SCHEMA, checkCsv, givenTypes, grouped, newEstimates, readEstimates, template, yearOf, type Estimates } from '../src/features/studio/population/estimates'
import { populationMessages as m } from '../src/i18n/messages'
import { parseJson } from '../src/platform/json'
import type { ApiRequest, ApiResponse, Transport } from '../src/platform/types'

// #553: a year's Population Estimates in the Studio, and the HMIS lead's CSV checked at central.

const estimates = (): Estimates => {
  const read = readEstimates(JSON.stringify(population))
  if ('refusal' in read) throw new Error(read.refusal)
  return read.estimates
}

describe('estimates', () => {
  test('the reference package reads, and the template has a column per given type', () => {
    const read = estimates()
    expect(read.year).toBe(2026)
    expect(givenTypes(read).map((t) => t.code)).toEqual(['total-population'])
    expect(template(read)).toBe('place,total-population\n')
  })

  test('a file that is not population/1 is refused', () => {
    expect(readEstimates('{')).toEqual({ refusal: 'not JSON' })
    expect(readEstimates('{"$schema":"x","year":2026,"types":[]}')).toEqual({ refusal: 'not written to population/1' })
    expect(readEstimates(`{"$schema":"${POPULATION_SCHEMA}","types":[]}`)).toEqual({ refusal: 'no year or no types' })
  })

  test('New writes the year’s estimates, expected pregnancies 5% of the total population', () => {
    expect(yearOf('Uganda population estimates 2027', 2000)).toBe(2027)
    expect(yearOf('Estimates', 2000)).toBe(2000)
    const made = newDesign('population', 'Population estimates 2027', 'https://health.go.ug/fhir', '0.1.0', [])
    expect(made.path).toBe(ESTIMATES_FILE)
    expect(made.resource).toEqual(newEstimates(2027))
    expect(newEstimates(2027).types[1]).toEqual({ code: 'expected-pregnancies', title: 'Expected pregnancies', derivedFrom: 'total-population', percent: '5' })
  })

  test('figures are grouped for reading', () => {
    expect(grouped('1797722', 'en')).toBe('1,797,722')
    expect(grouped('89886.1', 'en')).toBe('89,886')
    expect(grouped('n/a', 'en')).toBe('n/a')
  })
})

const transport = (status: number, body: unknown, sent: ApiRequest[] = []): Transport =>
  ({
    request: async (request: ApiRequest): Promise<ApiResponse> => {
      sent.push(request)
      return { status, headers: {}, body }
    },
  })

describe('checking a CSV', () => {
  test('central checks the file and the draft’s estimates; a refused file names its rows', async () => {
    const sent: ApiRequest[] = []
    const refused = await checkCsv(transport(422, { detail: 'x', problems: ['row 3: the place KAM999 is not in the facility registry or the NHFR'] }, sent), estimates(), 'place,total-population\n')
    expect(sent[0]).toMatchObject({ method: 'POST', path: '/places/population/preview', body: { csv: 'place,total-population\n', estimates: estimates() } })
    expect(refused).toEqual({ kind: 'refused', problems: ['row 3: the place KAM999 is not in the facility registry or the NHFR'] })
  })

  test('a checked file answers the estimates to save and each district’s totals', async () => {
    const districts = [{ code: 'district-kampala', name: 'Kampala', region: 'region-central', totals: [{ type: 'total-population', title: 'Total population', annual: '100000' }] }]
    // As the transport hands it over: parsed losslessly, numbers kept as their text.
    const body = parseJson(JSON.stringify({ estimates: estimates(), figures: 4, districts, problems: [] }))
    const checked = await checkCsv(transport(200, body), estimates(), 'place,total-population\n')
    expect(checked).toEqual({ kind: 'checked', estimates: estimates(), figures: 4, districts, problems: [] })
    expect(await checkCsv(transport(403, { detail: 'needs metadata.author' }), estimates(), '')).toEqual({ kind: 'failed', status: 403, detail: 'needs metadata.author' })
  })
})

test('every message formats with no ICU error', () => {
  const errors: string[] = []
  const intl = createIntl({ locale: 'en', onError: (e) => void errors.push(String(e)) })
  const values: Record<string, Record<string, string | number>> = {
    peUnreadable: { reason: 'r' },
    peHeading: { year: 2026 },
    peDerived: { percent: '5', from: 'Total population' },
    peFigures: { count: 4, places: 4 },
    peRefused: { count: 1 },
    peCheckFailed: { reason: 'r' },
    peDistricts: { count: 2 },
    peProblems: { count: 1 },
    peFailed: { reason: 'r' },
  }
  const texts = Object.entries(m).map(([key, message]) => intl.formatMessage({ id: message.id, defaultMessage: String(message.defaultMessage) }, values[key] ?? {}))
  expect(errors).toEqual([])
  expect(texts).toContain('4 figures for 4 places')
  expect(texts).toContain('5% of Total population')
  expect(intl.formatMessage(m.peFigures, { count: 0, places: 0 })).toBe('No figures yet.')
})
