import { stringifyJson } from '../../../platform/json'
import type { Transport } from '../../../platform/types'

// A year's Population Estimates (analytics.md § Age Band Sets and Population Estimates; ADR 0071;
// #553): `configuration/population.json` in a Population package (`ug.population.2026`), schema
// `population/1`. Estimate types are given per place or derived as a percentage of another type;
// figures are given at the lowest level available and summed upward at central, never split down.
// The HMIS lead imports a year's figures from a CSV template, which central checks against the
// facility registry before the figures are saved to the draft.

export const ESTIMATES_FILE = 'configuration/population.json'
export const POPULATION_SCHEMA = 'https://carapai.github.io/people-centered-healthcare/fhir/schema/population/1'

export interface EstimateType {
  code: string
  title?: string
  derivedFrom?: string
  /** A percentage of `derivedFrom`, as exact decimal text. */
  percent?: string
}

export interface Figure {
  place: string
  type: string
  value: number
}

export interface Estimates {
  $schema: string
  year: number
  types: EstimateType[]
  figures: Figure[]
}

/** The estimates a file holds, or why it cannot be read. */
export function readEstimates(text: string): { estimates: Estimates } | { refusal: string } {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return { refusal: 'not JSON' }
  }
  if (typeof value !== 'object' || value === null) return { refusal: 'not an object' }
  const record = value as Record<string, unknown>
  if (record['$schema'] !== POPULATION_SCHEMA) return { refusal: 'not written to population/1' }
  if (typeof record['year'] !== 'number' || !Array.isArray(record['types'])) return { refusal: 'no year or no types' }
  return { estimates: { ...(record as unknown as Estimates), figures: Array.isArray(record['figures']) ? (record['figures'] as Figure[]) : [] } }
}

/** The types given per place: the CSV template's columns. */
export const givenTypes = (estimates: Estimates) => estimates.types.filter((t) => t.derivedFrom === undefined)

/** The CSV template: a `place` column and one column per given type. */
export const template = (estimates: Estimates) => `${['place', ...givenTypes(estimates).map((t) => t.code)].join(',')}\n`

/** The year in a package name or title (`ug.population.2026`), else `fallback`. */
export function yearOf(text: string, fallback: number): number {
  const found = /(\d{4})(?!.*\d{4})/.exec(text)
  return found === null ? fallback : Number.parseInt(found[1]!, 10)
}

/** New estimates for `year`: the total population given per place and expected pregnancies
 * derived as 5% of it, as MoH does (ADR 0071). */
export const newEstimates = (year: number): Estimates => ({
  $schema: POPULATION_SCHEMA,
  year,
  types: [
    { code: 'total-population', title: 'Total population' },
    { code: 'expected-pregnancies', title: 'Expected pregnancies', derivedFrom: 'total-population', percent: '5' },
  ],
  figures: [],
})

/** One district's annual totals, given and derived; `annual` is null for *no estimate*. */
export interface DistrictTotals {
  code: string
  name: string
  region: string | null
  totals: { type: string; title: string; annual: string | null }[]
}

/** What central answers for an imported CSV. */
export type Checked =
  | { kind: 'checked'; estimates: Estimates; figures: number; districts: DistrictTotals[]; problems: string[] }
  | { kind: 'refused'; problems: string[] }
  | { kind: 'failed'; status: number; detail: string }

const strings = (value: unknown) => (Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [])

/** Checks `csv` against the facility registry for the draft's `estimates`
 * (`POST /api/places/population/preview`). Nothing is written. */
export async function checkCsv(transport: Transport, estimates: Estimates, csv: string): Promise<Checked> {
  const response = await transport.request({ method: 'POST', path: '/places/population/preview', body: { csv, estimates } })
  const body = (typeof response.body === 'object' && response.body !== null ? response.body : {}) as Record<string, unknown>
  if (response.status === 200) {
    // The body is parsed losslessly: read the estimates back as plain JSON, numbers as numbers.
    const read = readEstimates(stringifyJson(body['estimates'] ?? null))
    if ('refusal' in read) return { kind: 'failed', status: response.status, detail: read.refusal }
    return {
      kind: 'checked',
      estimates: read.estimates,
      figures: Number(String(body['figures'] ?? 0)),
      districts: (Array.isArray(body['districts']) ? body['districts'] : []) as DistrictTotals[],
      problems: strings(body['problems']),
    }
  }
  if (response.status === 422) return { kind: 'refused', problems: strings(body['problems']) }
  return { kind: 'failed', status: response.status, detail: typeof body['detail'] === 'string' ? body['detail'] : '' }
}

/** A whole or decimal number as text, grouped for reading (`1797722` → `1,797,722`). */
export function grouped(text: string, locale: string): string {
  const number = Number(text)
  return Number.isFinite(number) ? new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(number) : text
}
