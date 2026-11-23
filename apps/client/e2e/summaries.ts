// Sample indicator summaries for a local backend (#599): central-server receives them at startup
// (HEALTH_DEV_INDICATOR_SUMMARIES) as if the facilities' edges had synced them, since a local
// backend has no edges computing indicators. They are summary MeasureReports of the sample
// indicators (e2e/fixtures/indicators; malaria test positivity's for #600; ANC 1st visits by age band for #601), written as health-analytics-app's `report` writes them,
// for KAM013, KAM020 and WAK001, and Jinja's and Gulu's referral hospitals (JIN001, GUL001) so a
// national map shades the Eastern and Northern regions too (#604); KAM031 never syncs, so it
// shows as missing in Kampala.

const MEASURES = 'https://health.go.ug/fhir/Measure/'
export const FIRST_VISITS = `${MEASURES}sample-anc-first-visits|1.0.0`
export const BEFORE_12_WEEKS = `${MEASURES}sample-anc-first-visit-before-12-weeks|1.0.0`
export const GESTATIONAL_AGE = `${MEASURES}sample-anc-gestational-age-at-first-visit|1.0.0`
export const MALARIA_POSITIVITY = `${MEASURES}sample-malaria-test-positivity|1.0.0`
const PERIOD_TYPE = 'https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-period-type'
const POPULATION = 'http://terminology.hl7.org/CodeSystem/measure-population'

type Kind = 'weekly' | 'monthly' | 'quarterly' | 'financialJuly'
interface Period {
  kind: Kind
  id: string
  start: Date
  end: Date
}

const day = (d: Date) => d.toISOString().slice(0, 10)
const utc = (y: number, m: number, d: number) => new Date(Date.UTC(y, m, d))
const pad = (n: number) => String(n).padStart(2, '0')

/** The period of type `kind` holding `date`, as health-analytics names it. */
function periodOf(kind: Kind, date: Date): Period {
  const y = date.getUTCFullYear()
  const m = date.getUTCMonth()
  switch (kind) {
    case 'monthly':
      return { kind, id: `${y}-${pad(m + 1)}`, start: utc(y, m, 1), end: utc(y, m + 1, 0) }
    case 'quarterly': {
      const q = Math.floor(m / 3)
      return { kind, id: `${y}Q${q + 1}`, start: utc(y, q * 3, 1), end: utc(y, q * 3 + 3, 0) }
    }
    case 'financialJuly': {
      const from = m >= 6 ? y : y - 1
      return { kind, id: `${from}July`, start: utc(from, 6, 1), end: utc(from + 1, 6, 0) }
    }
    case 'weekly': {
      const weekday = date.getUTCDay() === 0 ? 7 : date.getUTCDay()
      const monday = utc(y, m, date.getUTCDate() - weekday + 1)
      const thursday = utc(monday.getUTCFullYear(), monday.getUTCMonth(), monday.getUTCDate() + 3)
      const year = thursday.getUTCFullYear()
      const week = Math.ceil(((thursday.getTime() - Date.UTC(year, 0, 1)) / 86_400_000 + 1) / 7)
      return { kind, id: `${year}W${pad(week)}`, start: monday, end: utc(monday.getUTCFullYear(), monday.getUTCMonth(), monday.getUTCDate() + 6) }
    }
  }
}

/** The last `count` periods of type `kind` up to the one holding `today`. */
function recent(kind: Kind, today: Date, count: number): Period[] {
  const periods: Period[] = []
  let at = today
  while (periods.length < count) {
    const period = periodOf(kind, at)
    periods.push(period)
    at = new Date(period.start.getTime() - 86_400_000)
  }
  return periods
}

const population = (code: string, count: number) => ({ code: { coding: [{ system: POPULATION, code }] }, count })

/** A made-up but steady figure for `facility` in `period`. */
function figure(facility: string, period: Period, scale: number): number {
  let hash = 7
  for (const c of `${facility}${period.id}`) hash = (hash * 31 + c.charCodeAt(0)) % 1009
  const days = (period.end.getTime() - period.start.getTime()) / 86_400_000 + 1
  return Math.max(1, Math.round(((scale + (hash % 7)) * days) / 30))
}

function report(measure: string, period: Period, facility: string, group: object, computed: Date) {
  return {
    measure,
    period_type: period.kind,
    period: period.id,
    facility,
    hash: 'dev-seed',
    report: {
      resourceType: 'MeasureReport',
      meta: { tag: [{ system: 'urn:health:facility', code: facility }] },
      extension: [{ url: PERIOD_TYPE, valueCode: period.kind }],
      status: 'complete',
      type: 'summary',
      measure,
      date: computed.toISOString().replace(/\.\d{3}Z$/, 'Z'),
      reporter: { identifier: { system: 'urn:health:facility', value: facility } },
      period: { start: day(period.start), end: day(period.end) },
      group: [group],
    },
  }
}

/** This quarter's malaria positives out of 20 outside the Central region (#604): Jinja 25% (*warning*), Gulu 10% (*good*). */
const ELSEWHERE_THIS_QUARTER: Record<string, number> = { JIN001: 5, GUL001: 2 }

/**
 * Malaria test positivity's share positive in `period`, out of 20: in quarters, 9 (45%, in the
 * *bad* band) this quarter and 6 (30%, a *warning*) the quarter before, at every facility of the
 * Central region, so Kampala's rolled-up figure is the same (#600), and the Eastern and Northern
 * regions' this quarter from `ELSEWHERE_THIS_QUARTER`; elsewhere a steady 3 to 9.
 */
function positiveOutOf20(facility: string, period: Period, today: Date): number {
  if (period.kind === 'quarterly') {
    const current = periodOf('quarterly', today)
    if (period.id === current.id) return ELSEWHERE_THIS_QUARTER[facility] ?? 9
    if (period.id === periodOf('quarterly', new Date(current.start.getTime() - 86_400_000)).id) return 6
  }
  return 3 + (figure(facility, period, 0) % 7)
}

/** The ANC age bands (the national core's age-bands-anc), then the band of a woman with no birth date. */
const AGE_BANDS = ['10-14', '15-19', '20-24', '25+', 'not-recorded']
/** Roughly how a facility's first visits fall across them, in twentieths. */
const AGE_SHARES = [1, 5, 7, 6]

/** `visits` split across the ANC age bands, adding up to `visits` (#601): one stratum per band. */
function byAgeBand(visits: number): object[] {
  const counts = AGE_SHARES.map((share) => Math.floor((visits * share) / 20))
  counts.push(visits - counts.reduce((a, b) => a + b, 0))
  return AGE_BANDS.map((code, i) => ({ value: { text: code }, population: [population('initial-population', counts[i] ?? 0)], measureScoreQuantity: { value: counts[i] ?? 0 } }))
}

/** Every sample summary, as `health_sync_protocol::IndicatorSummary` JSON, up to `today`. */
export function sampleSummaries(today: Date): object[] {
  const periods = [...recent('weekly', today, 16), ...recent('monthly', today, 12), ...recent('quarterly', today, 6), ...recent('financialJuly', today, 3)]
  const out: object[] = []
  for (const [facility, scale] of [
    ['KAM013', 14],
    ['KAM020', 9],
    ['WAK001', 20],
    ['JIN001', 25],
    ['GUL001', 12],
  ] as const) {
    for (const period of periods) {
      const computed = new Date(Math.min(today.getTime(), period.end.getTime() + 86_400_000 + 6 * 3_600_000))
      const visits = figure(facility, period, scale)
      const early = Math.round(visits * (0.3 + (visits % 5) / 20))
      const weeks = 14 + (visits % 9)
      out.push(report(FIRST_VISITS, period, facility, { population: [population('initial-population', visits)], measureScoreQuantity: { value: visits }, stratifier: [{ code: { text: 'Age band' }, stratum: byAgeBand(visits) }] }, computed))
      out.push(
        report(BEFORE_12_WEEKS, period, facility, { population: [population('numerator', early), population('denominator', visits)], measureScoreQuantity: { value: Math.round((early * 10_000) / visits) / 100 } }, computed),
      )
      out.push(report(GESTATIONAL_AGE, period, facility, { population: [population('measure-population', visits)], measureScoreQuantity: { value: weeks } }, computed))
      const tested = 20 * visits
      const positive = positiveOutOf20(facility, period, today) * visits
      out.push(
        report(MALARIA_POSITIVITY, period, facility, { population: [population('numerator', positive), population('denominator', tested)], measureScoreQuantity: { value: Math.round((positive * 10_000) / tested) / 100 } }, computed),
      )
    }
  }
  return out
}
