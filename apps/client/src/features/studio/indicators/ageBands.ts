// An Age Band Set in the Studio (analytics.md § Age Band Sets and Population Estimates; ADR 0071;
// #551): a `Library` of type `age-band-set` whose inline content is the table the author edits,
// its unit, the date age is taken at and its from–to rows. The editor reads and writes it here,
// flags gaps and overlaps as the author types, and works out the set's own version against the
// latest published one: a band, the unit or the age-at date changed is a major version, a rename
// a patch. Validation (`health-analytics`' `age_bands`) and the semver check remain the
// authority; this mirrors them so the table can say what is wrong before saving.

export const AGE_BAND_SET_TYPE = 'age-band-set'
export const AGE_BAND_SET_CONTENT = 'application/vnd.health.age-band-set+json'
const PLATFORM_CODES = 'https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/platform'

export type AgeUnit = 'days' | 'weeks' | 'months' | 'years'
export const ageUnits: readonly AgeUnit[] = ['days', 'weeks', 'months', 'years']
export type AgeAt = 'period-start' | 'period-end' | 'period-date'
export const ageAts: readonly AgeAt[] = ['period-end', 'period-start', 'period-date']

/** One row of the table: from an age to an age, both included; `to` null is open above. */
export interface Band {
  from: number | null
  to: number | null
}

/** What the editor edits. */
export interface AgeBandTable {
  title: string
  unit: AgeUnit
  ageAt: AgeAt
  bands: Band[]
}

type Json = Record<string, unknown>

const decode = (data: string) => new TextDecoder().decode(Uint8Array.from(atob(data), (c) => c.codePointAt(0) ?? 0))
const encode = (text: string) => btoa(Array.from(new TextEncoder().encode(text), (b) => String.fromCodePoint(b)).join(''))

const isObject = (value: unknown): value is Json => typeof value === 'object' && value !== null && !Array.isArray(value)

/** Whether `value` is an Age Band Set Library. */
export function isAgeBandSet(value: unknown): value is Json {
  if (!isObject(value) || value['resourceType'] !== 'Library') return false
  const type = value['type']
  const coding = isObject(type) && Array.isArray(type['coding']) ? type['coding'] : []
  return coding.some((c) => isObject(c) && c['code'] === AGE_BAND_SET_TYPE)
}

const wholeOrNull = (value: unknown): number | null => (typeof value === 'number' && Number.isInteger(value) ? value : null)

/** The table a Library holds, or why the editor cannot read it. */
export function readAgeBands(text: string): { library: Json; table: AgeBandTable } | { refusal: 'not-json' | 'not-age-band-set' | 'unreadable' } {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return { refusal: 'not-json' }
  }
  if (!isAgeBandSet(value)) return { refusal: 'not-age-band-set' }
  const content: unknown = Array.isArray(value['content']) ? value['content'][0] : undefined
  const data = isObject(content) ? content['data'] : undefined
  let held: unknown = {}
  if (typeof data === 'string') {
    try {
      held = JSON.parse(decode(data))
    } catch {
      return { refusal: 'unreadable' }
    }
  }
  if (!isObject(held)) return { refusal: 'unreadable' }
  const unit = ageUnits.find((u) => u === held['unit']) ?? 'years'
  const ageAt = ageAts.find((a) => a === held['ageAt']) ?? 'period-end'
  const bands = (Array.isArray(held['bands']) ? held['bands'] : []).map((b): Band => (isObject(b) ? { from: wholeOrNull(b['from']), to: wholeOrNull(b['to']) } : { from: null, to: null }))
  const title = typeof value['title'] === 'string' ? value['title'] : typeof value['name'] === 'string' ? value['name'] : ''
  return { library: value, table: { title, unit, ageAt, bands } }
}

/** The content the server reads: `{unit, ageAt, bands}`, an open band without `to`. */
export function contentOf(table: AgeBandTable): Json {
  return {
    unit: table.unit,
    ageAt: table.ageAt,
    bands: table.bands.map((b) => (b.to === null ? { from: b.from } : { from: b.from, to: b.to })),
  }
}

/** `library` holding `table` and at `version`, everything else as read. */
export function writeAgeBands(library: Json, table: AgeBandTable, version: string): Json {
  const content = Array.isArray(library['content']) ? library['content'] : []
  const first: Json = isObject(content[0]) ? content[0] : {}
  return {
    ...library,
    version,
    title: table.title,
    content: [{ ...first, contentType: AGE_BAND_SET_CONTENT, data: encode(`${JSON.stringify(contentOf(table), null, 2)}\n`) }, ...content.slice(1)],
  }
}

/** A new, empty Age Band Set Library: years at the end of the period, one row. */
export function newAgeBandSet(common: Json): Json {
  return writeAgeBands(
    { resourceType: 'Library', ...common, type: { coding: [{ system: PLATFORM_CODES, code: AGE_BAND_SET_TYPE }] }, content: [{ contentType: AGE_BAND_SET_CONTENT }] },
    { title: typeof common['title'] === 'string' ? common['title'] : '', unit: 'years', ageAt: 'period-end', bands: [{ from: 0, to: null }] },
    typeof common['version'] === 'string' ? common['version'] : '0.1.0',
  )
}

/** A band's label: `15–19 years`, `0 years`, `1 year`, `25+ years`. */
export function bandLabel(band: Band, unit: AgeUnit): string {
  const name = (n: number) => (n === 1 ? unit.replace(/s$/, '') : unit)
  if (band.from === null) return '?'
  if (band.to === null) return `${band.from}+ ${unit}`
  if (band.to === band.from) return `${band.to} ${name(band.to)}`
  return `${band.from}–${band.to} ${name(band.to)}`
}

/** What is wrong with the table, each naming the rows it concerns (0-based). */
export type BandProblem =
  | { kind: 'no-title' }
  | { kind: 'no-bands' }
  | { kind: 'no-from'; rows: [number] }
  | { kind: 'negative'; rows: [number] }
  | { kind: 'goes-down'; rows: [number] }
  | { kind: 'gap'; rows: [number, number]; from: number; to: number }
  | { kind: 'overlap'; rows: [number, number]; from: number; to: number | null }

/** The table's problems: as Validation refuses them, gaps and overlaps by the ages concerned. */
export function bandProblems(table: AgeBandTable): BandProblem[] {
  const problems: BandProblem[] = []
  if (table.title.trim() === '') problems.push({ kind: 'no-title' })
  if (table.bands.length === 0) problems.push({ kind: 'no-bands' })
  const usable: { row: number; from: number; to: number | null }[] = []
  table.bands.forEach((band, row) => {
    if (band.from === null) problems.push({ kind: 'no-from', rows: [row] })
    else if (band.from < 0) problems.push({ kind: 'negative', rows: [row] })
    else if (band.to !== null && band.to < band.from) problems.push({ kind: 'goes-down', rows: [row] })
    else usable.push({ row, from: band.from, to: band.to })
  })
  usable.sort((a, b) => a.from - b.from || (a.to ?? Infinity) - (b.to ?? Infinity))
  for (let i = 1; i < usable.length; i += 1) {
    const before = usable[i - 1]!
    const after = usable[i]!
    const rows: [number, number] = [before.row, after.row]
    if (before.to === null) problems.push({ kind: 'overlap', rows, from: after.from, to: after.to })
    else if (after.from <= before.to) problems.push({ kind: 'overlap', rows, from: after.from, to: after.to === null ? before.to : Math.min(before.to, after.to) })
    else if (after.from > before.to + 1) problems.push({ kind: 'gap', rows, from: before.to + 1, to: after.from - 1 })
  }
  return problems
}

/** The rows a problem is flagged on. */
export const flagged = (problems: readonly BandProblem[]) => new Set(problems.flatMap((p) => ('rows' in p ? p.rows : [])))

/** Why the set takes the version it does. */
export type VersionReason = 'new' | 'unchanged' | 'bands' | 'renamed'

/** The set's own version: a first version keeps `current`; against the latest published one,
 * changed bands, unit or age-at date take the next major, a new title the next patch, and
 * nothing changed keeps the published version. */
export function nextVersion(published: { version: string; table: AgeBandTable } | null, current: string, table: AgeBandTable): { version: string; reason: VersionReason } {
  if (published === null) return { version: current, reason: 'new' }
  const [major = 0, minor = 0, patch = 0] = published.version
    .split('-')[0]!
    .split('.')
    .map((n) => Number.parseInt(n, 10) || 0)
  if (JSON.stringify(contentOf(published.table)) !== JSON.stringify(contentOf(table))) return { version: `${major + 1}.0.0`, reason: 'bands' }
  if (published.table.title !== table.title) return { version: `${major}.${minor}.${patch + 1}`, reason: 'renamed' }
  return { version: published.version, reason: 'unchanged' }
}
