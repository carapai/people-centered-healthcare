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
      return { ...explorer, indicators: explorer.indicators.map((i) => ({ ...i, per: i.per === null ? null : Number(i.per) })) }
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

/** One bar series of the column chart: an indicator at a place. */
export interface Series {
  indicator: string
  place: string
  label: string
  /** By period, in the answer's order; null where there is no figure. */
  values: (number | null)[]
}

/** The column chart's series: each indicator at each place, the periods along the axis. */
export function chartSeries(answer: Answer, explorer: Explorer, question: Question): Series[] {
  const title = (m: string) => explorer.indicators.find((i) => i.measure === m)?.title ?? m
  const placeName = (code: string) => [explorer.place, ...explorer.places].find((p) => p?.code === code)?.name ?? code
  const series: Series[] = []
  for (const indicator of question.indicators) {
    for (const place of question.places) {
      const values = answer.periods.map((period) => {
        const cell = answer.cells.find((c) => c.indicator === indicator && c.place === place && c.period === period.id)
        return cell?.value == null ? null : Number(cell.value)
      })
      const label = question.places.length > 1 ? `${title(indicator)}, ${placeName(place)}` : title(indicator)
      series.push({ indicator, place, label, values })
    }
  }
  return series
}

/** The newest time any figure in `answer` was computed. */
export function newestComputed(answer: Answer): string | null {
  return answer.cells.reduce<string | null>((newest, c) => (c.computedAt != null && (newest === null || c.computedAt > newest) ? c.computedAt : newest), null)
}
