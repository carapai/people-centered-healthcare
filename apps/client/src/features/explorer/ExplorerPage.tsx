import { css, cx } from '@health/design-system/css'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { explorerMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { type Answer, type Explorer, ExplorerError, type ExplorerIndicator, NOT_RECORDED, type PeriodType, type Question, type Series, type SeriesBy, type TableLayout, type ThresholdBand, type Tone, type Visualization as Shown, type VisualizationType, answerQuery, chartSeries, childAreas, explorerQuery, fitted, newestComputed, previousPeriod, recentPeriods, sharedDisaggregations, singleNumbers, tableOf, toneOf } from './api'

const layout = css({ display: 'grid', gap: '4', gridTemplateColumns: { base: '1fr', md: '18rem 1fr' }, alignItems: 'start' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const panel = css({ display: 'flex', flexDirection: 'column', gap: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '3' })
const group = css({ display: 'flex', flexDirection: 'column', gap: '1', border: 'none', p: '0', m: '0', '& legend': { fontWeight: 'semibold', mb: '1' } })
const field = css({ border: '1px solid', borderColor: 'border.control', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const SERIES_COLORS = ['#1d4ed8', '#c2410c', '#15803d', '#7e22ce', '#0e7490', '#a16207', '#be123c', '#4d7c0f']
/** A threshold band's colour on a chart mark, and as a cell's background and text (4.5:1). */
const TONE_MARK: Record<Tone, string> = { good: '#15803d', warning: '#b45309', bad: '#b91c1c' }
const TONE_CELL: Record<Tone, { background: string; color: string }> = {
  good: { background: '#dcfce7', color: '#14532d' },
  warning: { background: '#fef3c7', color: '#78350f' },
  bad: { background: '#fee2e2', color: '#7f1d1d' },
}
const TYPES: VisualizationType[] = ['column', 'line', 'table', 'single']
const LAYOUTS: TableLayout[] = ['placesByIndicators', 'indicatorsByPeriods']
const SERIES: SeriesBy[] = ['indicator', 'place', 'band']
const pressable = css({
  border: '1px solid',
  borderColor: 'border.control',
  borderRadius: 'md',
  px: 'controlX',
  minHeight: 'control',
  bg: 'bg.surface',
  cursor: 'pointer',
  '&[aria-pressed=true]': { bg: 'bg.selected', color: 'fg.selected', borderColor: 'accent.default', fontWeight: 'semibold' },
})
const figures = css({ fontSize: 'sm', borderCollapse: 'collapse', '& th, & td': { textAlign: 'start', px: '2', py: '1', borderBottom: '1px solid', borderColor: 'border' } })
/** How many periods each range picker offers, newest last. */
const OFFERED: Record<PeriodType, number> = { weekly: 52, monthly: 24, quarterly: 12, financialJuly: 6 }
/** The range a period type starts with. */
const STARTING: Record<PeriodType, number> = { weekly: 12, monthly: 6, quarterly: 4, financialJuly: 3 }

const toggle = (list: string[], item: string) => (list.includes(item) ? list.filter((x) => x !== item) : [...list, item])


function defaultQuestion(explorer: Explorer, kind: PeriodType = 'monthly'): Question {
  const periods = recentPeriods(kind, new Date(), STARTING[kind])
  return {
    indicators: explorer.indicators.slice(0, 1).map((i) => i.measure),
    periodType: kind,
    from: periods[0] ?? '',
    to: periods[periods.length - 1] ?? '',
    places: explorer.place === null ? [] : [explorer.place.code],
  }
}

/**
 * The data explorer (ADR 0075; analytics.md § Dashboards; #599): pick indicators, a range of
 * standard periods and places in your area on the left, and see a live column chart of their
 * figures, rolled up at central from the facilities' summaries or computed on this facility's
 * edge, offline. The card says where and when the figures were computed, how many facilities
 * reported, which are missing (never counted as zero), and why a median is absent above
 * facility level.
 */
export function ExplorerPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const explorer = useQuery(explorerQuery(transport))
  if (explorer.data === undefined) {
    if (explorer.isPending) return <p role="status">{intl.formatMessage(m.exLoading)}</p>
    const forbidden = explorer.error instanceof ExplorerError && explorer.error.status === 403
    return (
      <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        <h1 className={heading}>{intl.formatMessage(m.exTitle)}</h1>
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(forbidden ? m.exForbidden : m.exFailed)}
        </p>
      </section>
    )
  }
  return <Explore explorer={explorer.data} />
}

function Explore({ explorer }: { explorer: Explorer }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  // The chart type and table layout are kept apart from the question: switching them keeps the selection.
  const [shown, setShownAs] = useState<Shown>(() => ({ type: 'column', question: defaultQuestion(explorer), layout: 'placesByIndicators', series: 'indicator' }))
  const setShown = (next: Shown) => setShownAs(fitted(explorer, next))
  const { question } = shown
  const setQuestion = (next: Question) => setShown({ ...shown, question: next })
  const splits = sharedDisaggregations(explorer, question.indicators)
  const children = childAreas(explorer)
  const ready = question.indicators.length > 0 && question.places.length > 0
  const answer = useQuery({ ...answerQuery(transport, question), enabled: ready, placeholderData: keepPreviousData })
  // A single number shows its change on the period before the range's last; ask for it when the range does not hold it.
  const earlier = previousPeriod(question.periodType, question.to)
  const askBefore = ready && shown.type === 'single' && question.from === question.to
  const before = useQuery({ ...answerQuery(transport, { ...question, from: earlier, to: earlier }), enabled: askBefore })
  const offered = recentPeriods(question.periodType, new Date(), OFFERED[question.periodType])
  const places = [explorer.place, ...explorer.places].filter((p) => p !== null)
  const periodTypeName = (kind: PeriodType) => intl.formatMessage(m[`exPeriodType_${kind}`])
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={heading}>{intl.formatMessage(m.exTitle)}</h1>
      <div className={layout}>
        <form className={panel} aria-label={intl.formatMessage(m.exPickers)} onSubmit={(e) => e.preventDefault()}>
          <fieldset className={group}>
            <legend>{intl.formatMessage(m.exData)}</legend>
            {explorer.indicators.length === 0 ? <p className={muted}>{intl.formatMessage(m.exNoIndicators)}</p> : null}
            {explorer.indicators.map((indicator) => (
              <label key={indicator.measure} className={css({ display: 'flex', gap: '2', alignItems: 'start' })}>
                <input type="checkbox" checked={question.indicators.includes(indicator.measure)} onChange={() => setQuestion({ ...question, indicators: toggle(question.indicators, indicator.measure) })} />
                <span>{indicator.title}</span>
              </label>
            ))}
          </fieldset>
          <fieldset className={group}>
            <legend>{intl.formatMessage(m.exPeriods)}</legend>
            <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              {intl.formatMessage(m.exPeriodType)}
              <select className={field} value={question.periodType} onChange={(e) => setQuestion({ ...defaultQuestion(explorer, e.target.value as PeriodType), indicators: question.indicators, places: question.places, ...(question.disaggregation === undefined ? {} : { disaggregation: question.disaggregation }) })}>
                {explorer.periodTypes.map((kind) => (
                  <option key={kind} value={kind}>
                    {periodTypeName(kind)}
                  </option>
                ))}
              </select>
            </label>
            <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              {intl.formatMessage(m.exFrom)}
              <select className={field} value={question.from} onChange={(e) => setQuestion({ ...question, from: e.target.value, to: offered.indexOf(question.to) < offered.indexOf(e.target.value) ? e.target.value : question.to })}>
                {offered.map((id) => (
                  <option key={id} value={id}>
                    {id}
                  </option>
                ))}
              </select>
            </label>
            <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              {intl.formatMessage(m.exTo)}
              <select className={field} value={question.to} onChange={(e) => setQuestion({ ...question, to: e.target.value, from: offered.indexOf(question.from) > offered.indexOf(e.target.value) ? e.target.value : question.from })}>
                {offered.map((id) => (
                  <option key={id} value={id}>
                    {id}
                  </option>
                ))}
              </select>
            </label>
          </fieldset>
          <fieldset className={group}>
            <legend>{intl.formatMessage(m.exPlaces)}</legend>
            {places.map((place) => (
              <label key={place.code} className={css({ display: 'flex', gap: '2', alignItems: 'start', pl: place.code === explorer.place?.code ? '0' : '4' })}>
                <input type="checkbox" checked={question.places.includes(place.code)} onChange={() => setQuestion({ ...question, places: toggle(question.places, place.code) })} />
                <span>{place.name}</span>
              </label>
            ))}
            {explorer.place !== null && children.length > 0 ? (
              <button type="button" className={pressable} aria-pressed={question.places.length === children.length && children.every((c) => question.places.includes(c))} onClick={() => setQuestion({ ...question, places: children })}>
                {intl.formatMessage(m.exChildAreas, { place: explorer.place.name })}
              </button>
            ) : null}
          </fieldset>
          <label className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontWeight: 'semibold' })}>
              {intl.formatMessage(m.exDisaggregation)}
              <select
                className={cx(field, css({ fontWeight: 'normal' }))}
                value={question.disaggregation ?? ''}
                onChange={(e) => {
                  const { disaggregation: _dropped, ...rest } = question
                  const name = e.target.value
                  setShown({ ...shown, question: name === '' ? rest : { ...rest, disaggregation: name }, series: name === '' ? (shown.series === 'band' ? 'indicator' : shown.series) : 'band' })
                }}
              >
                <option value="">{intl.formatMessage(m.exNoDisaggregation)}</option>
                {splits.map((d) => (
                  <option key={d.name} value={d.name}>
                    {d.name}
                  </option>
                ))}
              </select>
            </label>
          <fieldset className={group}>
            <legend>{intl.formatMessage(m.exSeriesBy)}</legend>
            <div className={css({ display: 'flex', gap: '2', flexWrap: 'wrap' })}>
              {SERIES.map((by) => (
                <button key={by} type="button" className={pressable} aria-pressed={shown.series === by} disabled={by === 'band' && question.disaggregation === undefined} onClick={() => setShown({ ...shown, series: by })}>
                  {intl.formatMessage(m[`exSeriesBy_${by}`])}
                </button>
              ))}
            </div>
          </fieldset>
        </form>
        <article className={panel} aria-label={intl.formatMessage(m.exVisualization)}>
          <div role="group" aria-label={intl.formatMessage(m.exChartType)} className={css({ display: 'flex', gap: '2', flexWrap: 'wrap' })}>
            {TYPES.map((type) => (
              <button key={type} type="button" className={pressable} aria-pressed={shown.type === type} onClick={() => setShown({ ...shown, type })}>
                {intl.formatMessage(m[`exType_${type}`])}
              </button>
            ))}
          </div>
          {shown.type === 'table' ? (
            <div role="group" aria-label={intl.formatMessage(m.exLayout)} className={css({ display: 'flex', gap: '2', flexWrap: 'wrap' })}>
              {LAYOUTS.map((tableLayout) => (
                <button key={tableLayout} type="button" className={pressable} aria-pressed={shown.layout === tableLayout} onClick={() => setShown({ ...shown, layout: tableLayout })}>
                  {intl.formatMessage(m[`exLayout_${tableLayout}`])}
                </button>
              ))}
            </div>
          ) : null}
          {!ready ? (
            <p className={muted}>{intl.formatMessage(m.exPickSomething)}</p>
          ) : answer.data === undefined ? (
            answer.isPending ? (
              <p role="status">{intl.formatMessage(m.exLoading)}</p>
            ) : (
              <p role="alert" className={css({ color: 'danger.text' })}>
                {answer.error instanceof ExplorerError && answer.error.detail !== undefined ? answer.error.detail : intl.formatMessage(m.exFailed)}
              </p>
            )
          ) : (
            <Visualization explorer={explorer} shown={shown} answer={answer.data} before={askBefore ? before.data : undefined} />
          )}
        </article>
      </div>
    </section>
  )
}

function Visualization({ explorer, shown, answer, before }: { explorer: Explorer; shown: Shown; answer: Answer; before: Answer | undefined }) {
  const intl = useIntl()
  const { question } = shown
  const bandName = (code: string) => (code === NOT_RECORDED ? intl.formatMessage(m.exNotRecorded) : code)
  const series = chartSeries(answer, explorer, question, shown.series, bandName)
  const newest = newestComputed(answer)
  const indicatorOf = (measure: string) => explorer.indicators.find((i) => i.measure === measure)
  const title = (measure: string) => indicatorOf(measure)?.title ?? measure
  const placeName = (code: string) => [explorer.place, ...explorer.places].find((p) => p?.code === code)?.name ?? code
  const facilityOnly = [...new Set(answer.cells.filter((c) => c.facilityOnly !== undefined).map((c) => c.indicator))]
  // Completeness is the same for every indicator a place's facilities sync: read it from the first that has figures.
  const counted = answer.cells.find((c) => c.facilityOnly === undefined)?.indicator
  const completeness = answer.cells.filter((c) => c.indicator === counted)
  const incomplete = completeness.filter((c) => (c.missing?.length ?? 0) > 0)
  const latest = answer.periods[answer.periods.length - 1]?.id
  const banded = question.indicators.map(indicatorOf).filter((i): i is ExplorerIndicator => i !== undefined && i.thresholds.length > 0)
  const chartLabel = { indicators: question.indicators.map(title).join(', '), from: question.from, to: question.to }
  const periods = answer.periods.map((p) => p.id)
  const show = (measure: string, value: number) => (indicatorOf(measure)?.calculation === 'percentage' ? intl.formatMessage(m.exPercent, { value: intl.formatNumber(value) }) : intl.formatNumber(value))
  return (
    <>
      {shown.type === 'column' ? <ColumnChart series={series} periods={periods} label={intl.formatMessage(m.exChartLabel, chartLabel)} /> : null}
      {shown.type === 'line' ? <LineChart series={series} periods={periods} label={intl.formatMessage(m.exLineChartLabel, chartLabel)} /> : null}
      {shown.type === 'single' ? (
        <ul aria-label={intl.formatMessage(m.exSingleNumbers)} className={css({ display: 'grid', gap: '3', gridTemplateColumns: 'repeat(auto-fill, minmax(14rem, 1fr))' })}>
          {singleNumbers(answer, before, explorer, question).map((n) => (
            <li key={`${n.indicator} ${n.place}`} aria-label={n.label} data-tone={n.tone ?? undefined} className={css({ display: 'flex', flexDirection: 'column', gap: '1', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '3' })} style={n.tone === null ? undefined : { ...TONE_CELL[n.tone], borderColor: TONE_MARK[n.tone] }}>
              <span className={css({ fontSize: 'sm' })}>
                {n.label}, {n.period}
              </span>
              <span className={css({ fontSize: '3xl', fontWeight: 'semibold' })}>{n.value === null ? '—' : show(n.indicator, n.value)}</span>
              {n.tone === null ? null : <span className={css({ fontSize: 'sm', fontWeight: 'semibold' })}>{intl.formatMessage(m[`exTone_${n.tone}`])}</span>}
              <span className={css({ fontSize: 'sm' })}>
                {n.change === null || n.previous === null
                  ? intl.formatMessage(m.exNoChange, { period: n.previousPeriod })
                  : intl.formatMessage(m.exChange, { change: intl.formatNumber(n.change, { signDisplay: 'exceptZero' }), period: n.previousPeriod, previous: show(n.indicator, n.previous) })}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
      {shown.type === 'single' ? null : <FiguresTable
          table={tableOf(answer, explorer, question, shown.type === 'table' ? shown.layout : 'indicatorsByPeriods', shown.series, bandName)}
          corner={intl.formatMessage(shown.type === 'table' && shown.layout === 'placesByIndicators' ? m.exPlace : shown.series === 'band' && question.disaggregation !== undefined ? m.exBandHeader : shown.series === 'place' ? m.exPlace : m.exSeries)}
        />}
      {banded.length > 0 ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
          {banded.map((indicator) => (
            <ul key={indicator.measure} aria-label={intl.formatMessage(m.exLegend, { indicator: indicator.title })} className={css({ display: 'flex', gap: '3', flexWrap: 'wrap', alignItems: 'center' })}>
              <li className={css({ fontWeight: 'semibold' })} aria-hidden="true">
                {intl.formatMessage(m.exLegend, { indicator: indicator.title })}
              </li>
              {indicator.thresholds.map((band) => (
                <li key={`${band.from} ${band.to}`} data-tone={band.tone} className={css({ display: 'inline-flex', gap: '1', alignItems: 'center' })}>
                  <span aria-hidden="true" style={{ width: 12, height: 12, background: TONE_MARK[band.tone], display: 'inline-block', borderRadius: 2 }} />
                  {intl.formatMessage(m.exBand, { range: bandRange(intl, band), tone: intl.formatMessage(m[`exTone_${band.tone}`]) })}
                </li>
              ))}
            </ul>
          ))}
        </div>
      ) : null}
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
        <p className={muted}>
          {intl.formatMessage(answer.where === 'central' ? m.exWhereCentral : m.exWhereFacility)}{' '}
          {newest === null ? intl.formatMessage(m.exNeverComputed) : intl.formatMessage(m.exAsOf, { at: intl.formatDate(newest, { dateStyle: 'medium', timeStyle: 'short' }) })}
        </p>
        <ul aria-label={intl.formatMessage(m.exCompletenessTitle)} className={muted}>
          {completeness
            .filter((c) => c.period === latest)
            .map((cell) => (
              <li key={cell.place}>{intl.formatMessage(m.exCompleteness, { period: cell.period, place: placeName(cell.place), reported: cell.reported ?? 0, expected: cell.expected ?? 0 })}</li>
            ))}
        </ul>
        {incomplete.length > 0 ? (
          <div role="note" aria-label={intl.formatMessage(m.exMissingTitle)} className={css({ color: 'danger.text' })}>
            <p>{intl.formatMessage(m.exMissingTitle)}</p>
            <ul className={css({ listStyle: 'disc', pl: '5' })}>
              {incomplete.map((cell) => (
                <li key={`${cell.period} ${cell.place}`}>
                  {intl.formatMessage(m.exMissingLine, {
                    period: cell.period,
                    place: placeName(cell.place),
                    facilities: (cell.missing ?? []).map((f) => (f.lastSync === null ? intl.formatMessage(m.exNeverSynced, { name: f.name }) : intl.formatMessage(m.exLastSynced, { name: f.name, at: intl.formatDate(f.lastSync, { dateStyle: 'medium' }) }))).join('; '),
                  })}
                </li>
              ))}
            </ul>
          </div>
        ) : null}
        {facilityOnly.map((measure) => (
          <p key={measure} role="note">
            {intl.formatMessage(m.exFacilityOnly, { indicator: title(measure) })}
          </p>
        ))}
      </div>
    </>
  )
}

/** A band's range as a person reads it: `below 20`, `20 to 40`, `40 and above`. */
function bandRange(intl: ReturnType<typeof useIntl>, band: ThresholdBand): string {
  const n = (text: string) => intl.formatNumber(Number(text))
  if (band.from === null && band.to !== null) return intl.formatMessage(m.exBandBelow, { to: n(band.to) })
  if (band.from !== null && band.to === null) return intl.formatMessage(m.exBandFrom, { from: n(band.from) })
  if (band.from !== null && band.to !== null) return intl.formatMessage(m.exBandBetween, { from: n(band.from), to: n(band.to) })
  return intl.formatMessage(m.exBandEvery)
}

/** The figures as a table, each coloured by its indicator's threshold band. */
function FiguresTable({ table, corner }: { table: ReturnType<typeof tableOf>; corner: string }) {
  const intl = useIntl()
  return (
    <table className={figures}>
      <caption className={css({ textAlign: 'start', fontWeight: 'semibold' })}>{intl.formatMessage(m.exFigures)}</caption>
      <thead>
        <tr>
          <th scope="col">{corner}</th>
          {table.columns.map((c) => (
            <th key={c.key} scope="col">
              {c.label}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {table.rows.map((row) => (
          <tr key={row.key}>
            <th scope="row">{row.label}</th>
            {row.cells.map((c, index) => (
              <td key={table.columns[index]?.key} data-tone={c.tone ?? undefined} style={c.tone === null ? undefined : TONE_CELL[c.tone]} title={c.tone === null ? undefined : intl.formatMessage(m[`exTone_${c.tone}`])}>
                {c.value === null ? '—' : intl.formatNumber(c.value)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/** The colour of a series' mark for `value`: its band's when its indicator has bands, else the series'. */
const markColour = (s: Series, k: number, value: number) => {
  const tone = toneOf(s.thresholds, value)
  return tone === null ? SERIES_COLORS[k % SERIES_COLORS.length] : TONE_MARK[tone]
}

/** The axes shared by the column and line charts: gridlines with their figures, and the periods along the bottom. */
function axesOf(series: Series[], periods: string[]) {
  const width = 640
  const height = 240
  const left = 48
  const bottom = 36
  const values = series.flatMap((s) => s.values).filter((v): v is number => v !== null)
  const most = Math.max(1, ...values) * 1.1
  const column = (width - left - 8) / Math.max(1, periods.length)
  const y = (value: number) => height - bottom - ((height - bottom - 8) * value) / most
  return { width, height, left, bottom, most, column, y }
}

function Axes({ axes, periods }: { axes: ReturnType<typeof axesOf>; periods: string[] }) {
  const intl = useIntl()
  const { width, height, left, bottom, most, column, y } = axes
  return (
    <>
      {[0, 0.5, 1].map((t) => (
        <g key={t}>
          <line x1={left} x2={width} y1={y(most * t)} y2={y(most * t)} stroke="#e5e7eb" />
          <text x={left - 4} y={y(most * t) + 4} fontSize="10" textAnchor="end" fill="#6b7280">
            {intl.formatNumber(Math.round(most * t))}
          </text>
        </g>
      ))}
      {periods.map((period, i) => (
        <text key={period} x={left + column * i + column / 2} y={height - bottom + 14} fontSize="10" textAnchor="middle" fill="#374151">
          {period}
        </text>
      ))}
    </>
  )
}

/** The series' names and colours, under a chart of more than one. */
function SeriesKey({ series }: { series: Series[] }) {
  if (series.length < 2) return null
  return (
    <figcaption className={css({ display: 'flex', gap: '3', flexWrap: 'wrap', fontSize: 'xs' })}>
      {series.map((s, k) => (
        <span key={`${s.indicator} ${s.place} ${s.band ?? ''}`} className={cx(css({ display: 'inline-flex', gap: '1', alignItems: 'center' }))}>
          <span aria-hidden="true" style={{ width: 10, height: 10, background: SERIES_COLORS[k % SERIES_COLORS.length], display: 'inline-block' }} />
          {s.label}
        </span>
      ))}
    </figcaption>
  )
}

/** A line chart: the periods along the bottom, a line per series, each point coloured by its band; a gap where there is no figure. */
function LineChart({ series, periods, label }: { series: Series[]; periods: string[]; label: string }) {
  const intl = useIntl()
  const axes = axesOf(series, periods)
  const { left, column, y, height, bottom } = axes
  const x = (i: number) => left + column * i + column / 2
  return (
    <figure className={css({ display: 'flex', flexDirection: 'column', gap: '2', m: '0' })}>
      <svg viewBox={`0 0 ${axes.width} ${axes.height}`} role="img" aria-label={label} className={css({ width: '100%', height: 'auto' })}>
        <Axes axes={axes} periods={periods} />
        {series.map((s, k) => {
          // A line breaks where a period has no figure: never drawn through a missing one as if it were known.
          const runs: string[][] = [[]]
          s.values.forEach((value, i) => {
            if (value === null) runs.push([])
            else runs[runs.length - 1]?.push(`${x(i)},${y(value)}`)
          })
          return (
            <g key={`${s.indicator} ${s.place} ${s.band ?? ''}`}>
              {runs
                .filter((run) => run.length > 1)
                .map((run) => (
                  <polyline key={run[0]} fill="none" stroke={SERIES_COLORS[k % SERIES_COLORS.length]} strokeWidth="2" points={run.join(' ')} />
                ))}
              {s.values.map((value, i) =>
                value === null ? (
                  <text key={periods[i]} x={x(i)} y={height - bottom - 4} fontSize="10" textAnchor="middle" fill="#b91c1c">
                    —
                  </text>
                ) : (
                  <circle key={periods[i]} cx={x(i)} cy={y(value)} r="4" fill={markColour(s, k, value)} stroke={SERIES_COLORS[k % SERIES_COLORS.length]} strokeWidth="1.5">
                    <title>{`${s.label}, ${periods[i]}: ${intl.formatNumber(value)}`}</title>
                  </circle>
                ),
              )}
            </g>
          )
        })}
      </svg>
      <SeriesKey series={series} />
    </figure>
  )
}

/** A column chart: the periods along the bottom, a bar per series in each, coloured by its band, a dash where there is no figure. */
function ColumnChart({ series, periods, label }: { series: Series[]; periods: string[]; label: string }) {
  const intl = useIntl()
  const axes = axesOf(series, periods)
  const { left, column, y, height, bottom } = axes
  const bar = Math.max(3, (column * 0.8) / Math.max(1, series.length))
  return (
    <figure className={css({ display: 'flex', flexDirection: 'column', gap: '2', m: '0' })}>
      <svg viewBox={`0 0 ${axes.width} ${axes.height}`} role="img" aria-label={label} className={css({ width: '100%', height: 'auto' })}>
        <Axes axes={axes} periods={periods} />
        {series.map((s, k) =>
          s.values.map((value, i) => {
            const x = left + column * i + column * 0.1 + bar * k
            if (value === null)
              return (
                <text key={`${k}-${periods[i]}`} x={x + bar / 2} y={height - bottom - 4} fontSize="10" textAnchor="middle" fill="#b91c1c">
                  —
                </text>
              )
            return (
              <rect key={`${k}-${periods[i]}`} x={x} y={y(value)} width={bar - 1} height={height - bottom - y(value)} fill={markColour(s, k, value)} stroke={s.thresholds.length > 0 ? SERIES_COLORS[k % SERIES_COLORS.length] : undefined} strokeWidth={s.thresholds.length > 0 && series.length > 1 ? 2 : 0}>
                <title>{`${s.label}, ${periods[i]}: ${intl.formatNumber(value)}`}</title>
              </rect>
            )
          }),
        )}
      </svg>
      <SeriesKey series={series} />
    </figure>
  )
}
