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
      return { ...explorer, indicators: explorer.indicators.map((i) => ({ ...i, per: i.per === null ? null : Number(i.per), thresholds: i.thresholds ?? [] })) }
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
      return { ...answer, cells: answer.cells.map((c) => ({ ...c, ...(c.reported === undefined ? {} : { reported: plainCount(c.reported) ?? 0 }), ...(c.expected === undefined ? {} : { expected: plainCount(c.expected) ?? 0 }) })) }
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

/** The kinds of Visualization the explorer shows (ADR 0075); a map comes later. */
export type VisualizationType = 'column' | 'line' | 'table' | 'single'

/** How a table lays out its figures. */
export type TableLayout = 'placesByIndicators' | 'indicatorsByPeriods'

/**
 * What a Visualization shows: the question asked and how its answer is drawn. Switching the type
 * keeps the question; a saved Visualization (#602) is this with a title.
 */
export interface Visualization {
  type: VisualizationType
  question: Question
  layout: TableLayout
}

/** One series of a chart: an indicator at a place. */
export interface Series {
  indicator: string
  place: string
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
 * own column when there is more than one), or each indicator at each place down the side and the
 * periods across.
 */
export function tableOf(answer: Answer, explorer: Explorer, question: Question, layout: TableLayout): Table {
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
    rows: question.indicators.flatMap((indicator) =>
      question.places.map((place) => ({
        key: `${indicator} ${place}`,
        label: question.places.length > 1 ? `${title(indicator)}, ${placeName(place)}` : title(indicator),
        cells: periods.map((period) => cell(indicator, period, place)),
      })),
    ),
  }
}

/** The chart's series: each indicator at each place, the periods along the axis. */
export function chartSeries(answer: Answer, explorer: Explorer, question: Question): Series[] {
  const { title, placeName, bands } = namer(explorer)
  const series: Series[] = []
  for (const indicator of question.indicators) {
    for (const place of question.places) {
      const values = answer.periods.map((period) => figureOf(answer, indicator, period.id, place))
      const label = question.places.length > 1 ? `${title(indicator)}, ${placeName(place)}` : title(indicator)
      series.push({ indicator, place, label, values, thresholds: bands(indicator) })
    }
  }
  return series
}

/** The newest time any figure in `answer` was computed. */
export function newestComputed(answer: Answer): string | null {
  return answer.cells.reduce<string | null>((newest, c) => (c.computedAt != null && (newest === null || c.computedAt > newest) ? c.computedAt : newest), null)
}
