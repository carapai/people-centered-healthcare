import type { Transport } from '../../platform/types'

/** The period types dashboards offer (ADR 0075): no arbitrary date ranges. */
export type PeriodType = 'weekly' | 'monthly' | 'quarterly' | 'financialJuly'

/** Where a place sits in the facility registry's hierarchy (ADR 0094). */
export type PlaceLevel = 'facility' | 'district' | 'region' | 'national'

/** A place the session may explore. */
export interface ExplorerPlace {
  code: string
  name: string
  level: PlaceLevel
  parent: string | null
}

/** An indicator offered (`health_analytics_app::explore::indicator_json`). */
export interface ExplorerIndicator {
  /** `url|version`. */
  measure: string
  title: string
  /** `count`, `percentage`, `rate`, `ratio`, or a value's aggregate (`sum`, `median`, …). */
  calculation: string
  per: number | null
  /** False for a median or a distinct count: they show at facility level only. */
  rollsUp: boolean
  /** Its threshold bands (a legend set), lowest first; none when it has none (#600). */
  thresholds: ThresholdBand[]
  /** What it is broken down by (#601): by an Age Band Set's bands or by a coded value's. */
  disaggregations: ExplorerDisaggregation[]
  /** The Population Estimate type its denominator is (#553), if it is not counted from records. */
  denominatorEstimate?: string | null
}

/** A disaggregation an indicator offers, by its name. */
export interface ExplorerDisaggregation {
  name: string
  by: 'age' | 'code'
}

/** The band every disaggregation has for a counted thing with no value (`health_analytics::NOT_RECORDED`). */
export const NOT_RECORDED = 'not-recorded'

/** The disaggregations every one of `indicators` offers, by name, in the first one's order: what the figures may be split by. */
export function sharedDisaggregations(explorer: Explorer, indicators: readonly string[]): ExplorerDisaggregation[] {
  const offered = indicators.map((m) => explorer.indicators.find((i) => i.measure === m)?.disaggregations ?? [])
  const [first, ...rest] = offered
  return (first ?? []).filter((d) => rest.every((other) => other.some((o) => o.name === d.name)))
}

/** How a figure in a threshold band reads. */
export type Tone = 'good' | 'warning' | 'bad'

/** A range of an indicator's figure: `from` (inclusive, none below every figure) up to `to` (exclusive, none above every figure). Bounds are exact decimal text. */
export interface ThresholdBand {
  from: string | null
  to: string | null
  tone: Tone
}

/** How `value` reads by `bands`: the tone of the band it falls in, if any (`health_analytics::tone_of`). */
export function toneOf(bands: readonly ThresholdBand[], value: number | null): Tone | null {
  if (value === null) return null
  return bands.find((b) => (b.from === null || value >= Number(b.from)) && (b.to === null || value < Number(b.to)))?.tone ?? null
}

/** What the session may explore (`GET /api/analytics/explorer`). */
export interface Explorer {
  /** `central` for summaries synced from the facilities, `facility` for this edge's own. */
  where: 'central' | 'facility'
  /** The session's place. */
  place: ExplorerPlace | null
  /** Every place below it. */
  places: ExplorerPlace[]
  indicators: ExplorerIndicator[]
  periodTypes: PeriodType[]
}

/** A question to the explorer. */
export interface Question {
  indicators: string[]
  periodType: PeriodType
  from: string
  to: string
  places: string[]
  /** The disaggregation each figure is split by (#601), one every indicator offers; none for totals only. */
  disaggregation?: string
}

/** A facility that has not reported for a period: missing, never zero. */
export interface MissingFacility {
  facility: string
  name: string
  /** When its summaries last synced, if ever. */
  lastSync: string | null
}

/** One indicator's figure for one period and place. */
export interface Cell {
  indicator: string
  period: string
  place: string
  /** Exact decimal text; null when no facility reported or the figure is facility level only. */
  value: string | null
  /** Set when a median or distinct count was asked above facility level. */
  facilityOnly?: string
  reported?: number
  expected?: number
  missing?: MissingFacility[]
  computedAt?: string | null
  /** Split by the question's disaggregation: each band's figure, in the summary's order (#601). */
  bands?: BandFigure[]
  /** For an indicator over a Population Estimate type (#553): what its facilities counted. */
  count?: number | null
  /** For an indicator over a Population Estimate type (#553): the place's estimate for the period. */
  estimate?: CellEstimate
}

/** The Population Estimate a figure's denominator is (#553; ADR 0071). */
export interface CellEstimate {
  /** The estimate type's code (`expected-pregnancies`). */
  type: string
  title: string
  /** The place's estimate for the period, pro rata, as exact decimal text; null when it has none. */
  value: string | null
}

/** Each indicator over a Population Estimate type in `answer` (#553), with the places that have no estimate for some period asked. */
export function estimateNotes(answer: Answer): { indicator: string; estimate: string; without: string[] }[] {
  const notes = new Map<string, { indicator: string; estimate: string; without: string[] }>()
  for (const cell of answer.cells) {
    if (cell.estimate === undefined) continue
    const note = notes.get(cell.indicator) ?? { indicator: cell.indicator, estimate: cell.estimate.title, without: [] }
    if (cell.estimate.value === null && !note.without.includes(cell.place)) note.without.push(cell.place)
    notes.set(cell.indicator, note)
  }
  return [...notes.values()]
}

/** One band's figure: recomputed at central from the band's own summed parts, never an average. */
export interface BandFigure {
  code: string
  /** Exact decimal text; null when the band's denominator is zero. */
  value: string | null
}

/** The explorer's answer (`POST /api/analytics/explore`). */
export interface Answer {
  where: 'central' | 'facility'
  periods: { id: string; start: string; end: string }[]
  cells: Cell[]
}

/** The explorer could not answer: the status and why, for the page to say. */
export class ExplorerError extends Error {
  constructor(
    readonly status: number,
    readonly detail: string | undefined,
  ) {
    super(`the explorer answered ${status}`)
    this.name = 'ExplorerError'
  }
}

function detailOf(body: unknown): string | undefined {
  return typeof body === 'object' && body !== null && 'detail' in body && typeof body.detail === 'string' ? body.detail : undefined
}

/** A count that arrived as an exact JSON number (lossless-json), as a plain number. */
const plainCount = (n: unknown) => (n === undefined || n === null ? undefined : Number(n))

/** What the signed-in session may explore. */
export function explorerQuery(transport: Transport) {
  return {
    queryKey: ['analytics', 'explorer'],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Explorer> => {
      const response = await transport.request({ method: 'GET', path: '/analytics/explorer', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new ExplorerError(response.status, detailOf(response.body))
      const explorer = response.body as Explorer
      return { ...explorer, indicators: explorer.indicators.map((i) => ({ ...i, per: i.per === null ? null : Number(i.per), thresholds: i.thresholds ?? [], disaggregations: i.disaggregations ?? [] })) }
    },
  }
}

/** The answer to `question`, asked again whenever it changes. */
export function answerQuery(transport: Transport, question: Question) {
  return {
    queryKey: ['analytics', 'explore', question],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Answer> => {
      const response = await transport.request({ method: 'POST', path: '/analytics/explore', body: question, ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new ExplorerError(response.status, detailOf(response.body))
      const answer = response.body as Answer
      // Counts arrive as exact JSON numbers (lossless-json): plain numbers here.
      return { ...answer, cells: answer.cells.map((c) => ({ ...c, ...(c.reported === undefined ? {} : { reported: plainCount(c.reported) ?? 0 }), ...(c.expected === undefined ? {} : { expected: plainCount(c.expected) ?? 0 }), ...(c.count === undefined ? {} : { count: plainCount(c.count) ?? null }) })) }
    },
  }
}

const pad = (n: number) => String(n).padStart(2, '0')

/** The ISO week-numbering year and week of `day` (a UTC date). */
function isoWeek(day: Date): { year: number; week: number } {
  const date = new Date(Date.UTC(day.getUTCFullYear(), day.getUTCMonth(), day.getUTCDate()))
  const weekday = date.getUTCDay() === 0 ? 7 : date.getUTCDay()
  date.setUTCDate(date.getUTCDate() + 4 - weekday)
  const yearStart = Date.UTC(date.getUTCFullYear(), 0, 1)
  return { year: date.getUTCFullYear(), week: Math.ceil(((date.getTime() - yearStart) / 86_400_000 + 1) / 7) }
}

/** The id of the period of type `kind` holding `day`, as the server writes it (`2026W10`, `2026-03`, `2026Q1`, `2025July`). */
export function periodOf(kind: PeriodType, day: Date): string {
  const year = day.getUTCFullYear()
  const month = day.getUTCMonth() + 1
  switch (kind) {
    case 'weekly': {
      const { year: weekYear, week } = isoWeek(day)
      return `${weekYear}W${pad(week)}`
    }
    case 'monthly':
      return `${year}-${pad(month)}`
    case 'quarterly':
      return `${year}Q${Math.floor((month - 1) / 3) + 1}`
    case 'financialJuly':
      return `${month >= 7 ? year : year - 1}July`
  }
}

const STEP_DAYS: Record<PeriodType, number> = { weekly: 7, monthly: 28, quarterly: 85, financialJuly: 360 }

/** The last `count` periods of type `kind` up to the one holding `today`, oldest first: what the range pickers offer. */
export function recentPeriods(kind: PeriodType, today: Date, count: number): string[] {
  const ids: string[] = []
  let day = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate()))
  while (ids.length < count) {
    const id = periodOf(kind, day)
    if (ids[0] !== id) ids.unshift(id)
    day = new Date(day.getTime() - STEP_DAYS[kind] * 86_400_000)
  }
  return ids
}

/** The period of type `kind` before the one with id `id`. */
export function previousPeriod(kind: PeriodType, id: string): string {
  const year = Number(id.slice(0, 4))
  switch (kind) {
    case 'monthly': {
      const month = Number(id.slice(5, 7))
      return month === 1 ? `${year - 1}-12` : `${year}-${pad(month - 1)}`
    }
    case 'quarterly': {
      const quarter = Number(id.slice(5))
      return quarter === 1 ? `${year - 1}Q4` : `${year}Q${quarter - 1}`
    }
    case 'financialJuly':
      return `${year - 1}July`
    case 'weekly': {
      // The Monday of week 1 is the Monday on or before 4 January; step back a week from this one's Monday.
      const january4 = new Date(Date.UTC(year, 0, 4))
      const weekday = january4.getUTCDay() === 0 ? 7 : january4.getUTCDay()
      const monday = Date.UTC(year, 0, 4 - weekday + 1 + (Number(id.slice(5)) - 1) * 7)
      return periodOf('weekly', new Date(monday - 7 * 86_400_000))
    }
  }
}

/** The kinds of Visualization the explorer shows (ADR 0075): a map (#604) shades boundaries and facility points by band. */
export type VisualizationType = 'column' | 'line' | 'table' | 'single' | 'map'

/** How a table lays out its figures. */
export type TableLayout = 'placesByIndicators' | 'indicatorsByPeriods'

/** What a chart's series are (#601): the indicators, the places, or the bands of the question's disaggregation. */
export type SeriesBy = 'indicator' | 'place' | 'band'

/**
 * Whose places a Visualization shows when they follow the viewer (#602): the viewer's own place,
 * or each area directly under it. A saved one shared with another district shows that district's.
 */
export type Follow = 'myPlace' | 'eachAreaUnder'

/**
 * What a Visualization shows: the question asked and how its answer is drawn. Switching the type
 * keeps the question; a saved Visualization (#602) is this with a title.
 */
export interface Visualization {
  type: VisualizationType
  question: Question
  layout: TableLayout
  series: SeriesBy
  /** Set when the places follow the viewer rather than name places; picking places clears it. */
  follow?: Follow
}

/**
 * `shown` as `explorer`'s session sees it (#602): when it follows the viewer, its places are the
 * session's own place or each area under it, whoever saved it; otherwise as saved.
 */
export function forViewer(explorer: Explorer, shown: Visualization): Visualization {
  if (shown.follow === undefined) return shown
  const mine = explorer.place === null ? [] : [explorer.place.code]
  // At a facility there is no area under it: each area under it is the facility itself, as on an edge (#879).
  const places = shown.follow === 'myPlace' || explorer.place?.level === 'facility' ? mine : childAreas(explorer)
  return { ...shown, question: { ...shown.question, places } }
}

/** One series of a chart: an indicator at a place, or one band of it. */
export interface Series {
  indicator: string
  place: string
  /** The disaggregation band, when the series are bands. */
  band: string | null
  label: string
  /** By period, in the answer's order; null where there is no figure. */
  values: (number | null)[]
  /** The indicator's threshold bands, which colour its figures. */
  thresholds: ThresholdBand[]
}

/** Names an answer's indicators and places, and finds an indicator's bands. */
function namer(explorer: Explorer) {
  const indicator = (m: string) => explorer.indicators.find((i) => i.measure === m)
  return {
    title: (m: string) => indicator(m)?.title ?? m,
    placeName: (code: string) => [explorer.place, ...explorer.places].find((p) => p?.code === code)?.name ?? code,
    bands: (m: string) => indicator(m)?.thresholds ?? [],
  }
}

/** The figure of `answer` for an indicator, period and place, as a number; null where there is none. */
function figureOf(answer: Answer, indicator: string, period: string, place: string): number | null {
  const cell = answer.cells.find((c) => c.indicator === indicator && c.place === place && c.period === period)
  return cell?.value == null ? null : Number(cell.value)
}

/** A single number: an indicator's figure at a place for the last period, with its change on the period before. */
export interface SingleNumber {
  indicator: string
  place: string
  label: string
  period: string
  value: number | null
  previousPeriod: string
  previous: number | null
  /** `value − previous`; null unless both have a figure. */
  change: number | null
  tone: Tone | null
}

/**
 * The single numbers of `answer`, one per indicator and place: each one's figure for the range's
 * last period, and its change on the period of the same type before, read from `answer` when the
 * range holds it and from `before` (the answer for that period alone) otherwise.
 */
export function singleNumbers(answer: Answer, before: Answer | undefined, explorer: Explorer, question: Question): SingleNumber[] {
  const { title, placeName, bands } = namer(explorer)
  const period = answer.periods[answer.periods.length - 1]?.id ?? question.to
  const earlier = previousPeriod(question.periodType, period)
  const from = answer.periods.some((p) => p.id === earlier) ? answer : before
  return question.indicators.flatMap((indicator) =>
    question.places.map((place) => {
      const value = figureOf(answer, indicator, period, place)
      const previous = from === undefined ? null : figureOf(from, indicator, earlier, place)
      const label = question.places.length > 1 ? `${title(indicator)}, ${placeName(place)}` : title(indicator)
      return { indicator, place, label, period, value, previousPeriod: earlier, previous, change: value === null || previous === null ? null : roundExact(value - previous), tone: toneOf(bands(indicator), value) }
    }),
  )
}

/** A difference of two decimal figures without binary noise (45 − 30.1 is 14.9, not 14.899999…). */
const roundExact = (n: number) => Math.round(n * 1e6) / 1e6

/** One figure in a table, with its tone. */
export interface TableCell {
  indicator: string
  value: number | null
  tone: Tone | null
}

/** A table of figures: labelled columns, and rows of cells in the same order. */
export interface Table {
  columns: { key: string; label: string }[]
  rows: { key: string; label: string; cells: TableCell[] }[]
}

/**
 * The figures of `answer` as a table: places down the side and indicators across (each period's
 * own column when there is more than one), or the chart's series (`by`, #601) down the side and
 * the periods across.
 */
export function tableOf(answer: Answer, explorer: Explorer, question: Question, layout: TableLayout, by: SeriesBy = 'indicator', bandName: (code: string) => string = (c) => c): Table {
  const { title, placeName, bands } = namer(explorer)
  const cell = (indicator: string, period: string, place: string): TableCell => {
    const value = figureOf(answer, indicator, period, place)
    return { indicator, value, tone: toneOf(bands(indicator), value) }
  }
  const periods = answer.periods.map((p) => p.id)
  if (layout === 'placesByIndicators') {
    const columns = question.indicators.flatMap((indicator) => periods.map((period) => ({ indicator, period })))
    return {
      columns: columns.map(({ indicator, period }) => ({ key: `${indicator} ${period}`, label: periods.length > 1 ? `${title(indicator)}, ${period}` : title(indicator) })),
      rows: question.places.map((place) => ({ key: place, label: placeName(place), cells: columns.map(({ indicator, period }) => cell(indicator, period, place)) })),
    }
  }
  return {
    columns: periods.map((period) => ({ key: period, label: period })),
    rows: chartSeries(answer, explorer, question, by, bandName).map((s) => ({
      key: `${s.indicator} ${s.place} ${s.band ?? ''}`,
      label: s.label,
      cells: s.values.map((value) => ({ indicator: s.indicator, value, tone: toneOf(s.thresholds, value) })),
    })),
  }
}

/** The band codes `answer` has for `indicator`, in the order the summaries give them, *not recorded* last. */
export function bandsOf(answer: Answer, indicator: string): string[] {
  const codes: string[] = []
  for (const cell of answer.cells) {
    if (cell.indicator !== indicator) continue
    for (const band of cell.bands ?? []) if (!codes.includes(band.code)) codes.push(band.code)
  }
  return [...codes.filter((c) => c !== NOT_RECORDED), ...codes.filter((c) => c === NOT_RECORDED)]
}

/** A band's figure in `answer` for an indicator, period and place, as a number; null where there is none. */
function bandFigureOf(answer: Answer, indicator: string, period: string, place: string, band: string): number | null {
  const cell = answer.cells.find((c) => c.indicator === indicator && c.place === place && c.period === period)
  const value = cell?.bands?.find((b) => b.code === band)?.value
  return value == null ? null : Number(value)
}

/**
 * The chart's series, the periods along the axis (#601): by indicator, each indicator at each
 * place; by place, each place for each indicator, named by the place; by band, each band of the
 * question's disaggregation for each indicator at each place, named by the band. A label names
 * the other dimensions only when the question has more than one of them. `bandName` names a band
 * (*not recorded* is the page's to translate).
 */
export function chartSeries(answer: Answer, explorer: Explorer, question: Question, by: SeriesBy = 'indicator', bandName: (code: string) => string = (c) => c): Series[] {
  const { title, placeName, bands } = namer(explorer)
  const manyIndicators = question.indicators.length > 1
  const manyPlaces = question.places.length > 1
  const series: Series[] = []
  for (const indicator of question.indicators) {
    for (const place of question.places) {
      const thresholds = bands(indicator)
      if (by === 'band' && question.disaggregation !== undefined) {
        for (const band of bandsOf(answer, indicator)) {
          const values = answer.periods.map((period) => bandFigureOf(answer, indicator, period.id, place, band))
          const label = [bandName(band), ...(manyIndicators ? [title(indicator)] : []), ...(manyPlaces ? [placeName(place)] : [])].join(', ')
          series.push({ indicator, place, band, label, values, thresholds })
        }
        continue
      }
      const values = answer.periods.map((period) => figureOf(answer, indicator, period.id, place))
      const label = by === 'place' ? (manyIndicators ? `${placeName(place)}, ${title(indicator)}` : placeName(place)) : manyPlaces ? `${title(indicator)}, ${placeName(place)}` : title(indicator)
      series.push({ indicator, place, band: null, label, values, thresholds })
    }
  }
  return series
}

/**
 * `shown` with a question the explorer can answer (#601): the disaggregation is dropped once an
 * indicator picked does not offer it, and band series need one.
 */
export function fitted(explorer: Explorer, shown: Visualization): Visualization {
  const { disaggregation, ...rest } = shown.question
  const kept = disaggregation !== undefined && sharedDisaggregations(explorer, rest.indicators).some((d) => d.name === disaggregation)
  return { ...shown, question: kept ? { ...rest, disaggregation } : rest, series: !kept && shown.series === 'band' ? 'indicator' : shown.series }
}

/** The places directly under `place` in the session's area: what *each area under my place* picks. */
export function childAreas(explorer: Explorer): string[] {
  const root = explorer.place?.code
  return root === undefined ? [] : explorer.places.filter((p) => p.parent === root).map((p) => p.code)
}

/** The newest time any figure in `answer` was computed. */
export function newestComputed(answer: Answer): string | null {
  return answer.cells.reduce<string | null>((newest, c) => (c.computedAt != null && (newest === null || c.computedAt > newest) ? c.computedAt : newest), null)
}
