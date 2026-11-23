// A standard Dashboard as the Dashboard designer edits it (studio.md § Dashboards; analytics.md §
// Dashboards; ADR 0075; #603, #879): `configuration/dashboards/<id>.json` in a Reporting package,
// written to the release's `dashboard/1` schema and read on the server as
// `health_analytics::StandardDashboard`. This module reads the file, edits it one step at a time
// and writes it back in the schema's key order, so a Dashboard built here is the same definition
// as one written by hand. Validation is the server's (`POST …/dashboards/preview` answers
// publication's problems as the author edits); nothing here decides what is valid.

import type { StudioDashboardPreview } from '@health/api-client'

import type { Answer, Explorer, Follow, PeriodType, SeriesBy, TableLayout, ThresholdBand, Visualization, VisualizationType } from '../../explorer/api'
import { slug, unique } from '../reports/report'

export const DASHBOARD_SCHEMA = 'https://carapai.github.io/people-centered-healthcare/fhir/schema/dashboard/1'
export const DASHBOARDS_FOLDER = 'configuration/dashboards/'

/** What a standard Dashboard offers (`health_analytics::standard`). */
export const TYPES: readonly VisualizationType[] = ['column', 'line', 'table', 'single', 'map']
export const LAYOUTS: readonly TableLayout[] = ['placesByIndicators', 'indicatorsByPeriods']
export const SERIES: readonly SeriesBy[] = ['indicator', 'place', 'band']
export const FOLLOWS: readonly Follow[] = ['myPlace', 'eachAreaUnder']
export const PERIOD_TYPES: readonly PeriodType[] = ['weekly', 'monthly', 'quarterly', 'financialJuly']
/** The most periods "last n" shows (`health_analytics::MOST_PERIODS`). */
export const MOST_PERIODS = 106
/** The most indicators one Visualization shows. */
export const MOST_INDICATORS = 12

/** The periods a Visualization shows: the last `last` of its type up to today, or a fixed range. */
export type Periods = { last: number } | { from: string; to: string }

export interface Question {
  /** `url|version` each. */
  indicators: string[]
  periodType: PeriodType
  periods: Periods
  disaggregation?: string
}

export interface Item {
  id: string
  title: string
  type: VisualizationType
  question: Question
  layout: TableLayout
  series: SeriesBy
  follow: Follow
}

export interface Dashboard {
  id: string
  title: string
  description?: string
  items: Item[]
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v)
const str = (v: unknown) => (typeof v === 'string' ? v : '')
const oneOf = <T extends string>(allowed: readonly T[], v: unknown, otherwise: T): T => (allowed.includes(v as T) ? (v as T) : otherwise)

export const newDashboard = (id: string, title: string): Dashboard => ({ id, title, items: [] })

/** Where Dashboard `id` lives in its package. */
export const dashboardPath = (id: string) => `${DASHBOARDS_FOLDER}${id}.json`

/** The Dashboard in `text`, or why the designer cannot open it. */
export function readDashboard(text: string): { dashboard: Dashboard } | { refusal: string } {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return { refusal: 'not JSON' }
  }
  if (!isObject(value)) return { refusal: 'not a JSON object' }
  if (value['$schema'] !== DASHBOARD_SCHEMA) return { refusal: 'not written to the dashboard/1 schema' }
  const items = Array.isArray(value['items']) ? (value['items'] as unknown[]) : []
  if (!items.every((i) => isObject(i) && isObject(i['visualization']) && isObject(i['visualization']['question']))) return { refusal: 'an item has no visualization' }
  const dashboard: Dashboard = {
    id: str(value['id']),
    title: str(value['title']),
    items: (items as Record<string, Record<string, Record<string, unknown>>>[]).map((i): Item => {
      const shown = i['visualization'] as Record<string, unknown>
      const q = shown['question'] as Record<string, unknown>
      const periods: Periods = typeof q['last'] === 'number' ? { last: q['last'] } : { from: str(q['from']), to: str(q['to']) }
      const question: Question = {
        indicators: Array.isArray(q['indicators']) ? (q['indicators'] as unknown[]).filter((m): m is string => typeof m === 'string') : [],
        periodType: oneOf(PERIOD_TYPES, q['periodType'], 'monthly'),
        periods,
      }
      if (typeof q['disaggregation'] === 'string') question.disaggregation = q['disaggregation']
      return {
        id: str(i['id']),
        title: str(i['title']),
        type: oneOf(TYPES, shown['type'], 'column'),
        question,
        layout: oneOf(LAYOUTS, shown['layout'], 'placesByIndicators'),
        series: oneOf(SERIES, shown['series'], 'indicator'),
        follow: oneOf(FOLLOWS, shown['follow'], 'myPlace'),
      }
    }),
  }
  if (typeof value['description'] === 'string') dashboard.description = value['description']
  return { dashboard }
}

/** The file for `dashboard`, in the schema's key order. */
export function writeDashboard(dashboard: Dashboard): Record<string, unknown> {
  return {
    $schema: DASHBOARD_SCHEMA,
    id: dashboard.id,
    title: dashboard.title,
    ...(dashboard.description === undefined ? {} : { description: dashboard.description }),
    items: dashboard.items.map((item) => ({
      id: item.id,
      title: item.title,
      visualization: {
        type: item.type,
        question: {
          indicators: item.question.indicators,
          periodType: item.question.periodType,
          ...('last' in item.question.periods ? { last: item.question.periods.last } : { from: item.question.periods.from, to: item.question.periods.to }),
          ...(item.question.disaggregation === undefined ? {} : { disaggregation: item.question.disaggregation }),
        },
        layout: item.layout,
        series: item.series,
        follow: item.follow,
      },
    })),
  }
}

export const withSettings = (dashboard: Dashboard, next: Partial<Pick<Dashboard, 'title' | 'description'>>): Dashboard => {
  const out = { ...dashboard, ...next }
  if (out.description !== undefined && out.description.trim() === '') delete out.description
  return out
}

/** `dashboard` with a new column chart titled `title`, over `indicator` if given, for the last 6 months, at the end. */
export function addItem(dashboard: Dashboard, title: string, indicator?: string): Dashboard {
  const id = unique(
    slug(title),
    dashboard.items.map((i) => i.id),
  )
  const item: Item = {
    id,
    title,
    type: 'column',
    question: { indicators: indicator === undefined || indicator === '' ? [] : [indicator], periodType: 'monthly', periods: { last: 6 } },
    layout: 'placesByIndicators',
    series: 'indicator',
    follow: 'myPlace',
  }
  return { ...dashboard, items: [...dashboard.items, item] }
}

export const removeItem = (dashboard: Dashboard, at: number): Dashboard => ({ ...dashboard, items: dashboard.items.filter((_, n) => n !== at) })

/** `dashboard` with item `at` moved up (-1) or down (1). */
export function moveItem(dashboard: Dashboard, at: number, by: -1 | 1): Dashboard {
  const to = at + by
  if (to < 0 || to >= dashboard.items.length) return dashboard
  const items = [...dashboard.items]
  const [moved] = items.splice(at, 1)
  if (moved !== undefined) items.splice(to, 0, moved)
  return { ...dashboard, items }
}

export const updateItem = (dashboard: Dashboard, at: number, change: (item: Item) => Item): Dashboard => ({ ...dashboard, items: dashboard.items.map((i, n) => (n === at ? change(i) : i)) })

export const withItem = (item: Item, next: Partial<Pick<Item, 'id' | 'title' | 'type' | 'layout' | 'series' | 'follow'>>): Item => ({ ...item, ...next })

/** `item` showing `indicator` too, or no longer. A band series needs a disaggregation, so it is kept only while one is. */
export function toggleIndicator(item: Item, indicator: string, on: boolean): Item {
  const has = item.question.indicators.includes(indicator)
  if (on === has) return item
  const indicators = on ? [...item.question.indicators, indicator] : item.question.indicators.filter((m) => m !== indicator)
  return { ...item, question: { ...item.question, indicators } }
}

export const withPeriodType = (item: Item, periodType: PeriodType): Item => ({ ...item, question: { ...item.question, periodType, periods: 'last' in item.question.periods ? item.question.periods : { last: 6 } } })

export const withPeriods = (item: Item, periods: Periods): Item => ({ ...item, question: { ...item.question, periods } })

/** `item` split by disaggregation `name`, or by none; band series fall back to indicators without one. */
export function withDisaggregation(item: Item, name: string | undefined): Item {
  const { disaggregation: _dropped, ...rest } = item.question
  return { ...item, question: name === undefined || name === '' ? rest : { ...rest, disaggregation: name }, series: (name === undefined || name === '') && item.series === 'band' ? 'indicator' : item.series }
}

/** What an item's line in the outline says. */
export function summaryOf(item: Item): { type: VisualizationType; indicators: number; periods: string } {
  const p = item.question.periods
  return { type: item.type, indicators: item.question.indicators.length, periods: 'last' in p ? `last ${p.last} ${item.question.periodType}` : `${p.from} – ${p.to}` }
}

/** The place the preview's sample data stands for (`studio::dashboards::SAMPLE`). */
export const SAMPLE = 'sample'

/** The preview as the explorer's own types: a session at the sample place, with the indicators found. */
export function previewExplorer(preview: StudioDashboardPreview, sampleName: string): Explorer {
  return {
    where: 'facility',
    place: { code: SAMPLE, name: sampleName, level: 'facility', parent: null },
    places: [],
    periodTypes: [...PERIOD_TYPES],
    indicators: preview.indicators.map((i) => ({
      measure: i.measure,
      title: i.title,
      calculation: i.calculation,
      per: i.per === null || i.per === undefined ? null : Number(i.per),
      rollsUp: i.rollsUp,
      thresholds: i.thresholds.map((t): ThresholdBand => ({ from: t.from ?? null, to: t.to ?? null, tone: t.tone === 'good' || t.tone === 'warning' ? t.tone : 'bad' })),
      disaggregations: i.disaggregations.map((d) => ({ name: d.name, by: d.by === 'age' ? 'age' : 'code' })),
    })),
  }
}

type PreviewAnswer = NonNullable<StudioDashboardPreview['items'][number]['answer']>

/** A preview answer as the explorer's: each figure at the sample place, completely reported. */
export function previewAnswer(answer: PreviewAnswer): Answer {
  return {
    where: 'facility',
    periods: answer.periods,
    cells: answer.cells.map((c) => ({
      indicator: c.indicator,
      period: c.period,
      place: c.place,
      value: c.value ?? null,
      reported: 1,
      expected: 1,
      missing: [],
      computedAt: null,
      ...(c.bands === null || c.bands === undefined ? {} : { bands: c.bands.map((b) => ({ code: b.code, value: b.value ?? null })) }),
    })),
  }
}

/** Item `item` as the explorer shows it over the preview's periods `from`–`to`, its places the sample whatever it follows. A map is shown as a table: the sample has no place on a map. */
export function previewShown(item: Item, from: string, to: string): Visualization {
  return {
    type: item.type === 'map' ? 'table' : item.type,
    question: {
      indicators: item.question.indicators,
      periodType: item.question.periodType,
      from,
      to,
      places: [SAMPLE],
      ...(item.question.disaggregation === undefined ? {} : { disaggregation: item.question.disaggregation }),
    },
    layout: item.layout,
    series: item.series,
  }
}
