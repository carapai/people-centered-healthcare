import { css, cx } from '@health/design-system/css'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { explorerMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { type Answer, type Explorer, ExplorerError, type PeriodType, type Question, type Series, answerQuery, chartSeries, explorerQuery, newestComputed, recentPeriods } from './api'

const layout = css({ display: 'grid', gap: '4', gridTemplateColumns: { base: '1fr', md: '18rem 1fr' }, alignItems: 'start' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const panel = css({ display: 'flex', flexDirection: 'column', gap: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '3' })
const group = css({ display: 'flex', flexDirection: 'column', gap: '1', border: 'none', p: '0', m: '0', '& legend': { fontWeight: 'semibold', mb: '1' } })
const field = css({ border: '1px solid', borderColor: 'border.control', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const SERIES_COLORS = ['#1d4ed8', '#c2410c', '#15803d', '#7e22ce', '#0e7490', '#a16207', '#be123c', '#4d7c0f']
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
  const [question, setQuestion] = useState<Question>(() => defaultQuestion(explorer))
  const ready = question.indicators.length > 0 && question.places.length > 0
  const answer = useQuery({ ...answerQuery(transport, question), enabled: ready, placeholderData: keepPreviousData })
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
              <select className={field} value={question.periodType} onChange={(e) => setQuestion({ ...defaultQuestion(explorer, e.target.value as PeriodType), indicators: question.indicators, places: question.places })}>
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
          </fieldset>
        </form>
        <article className={panel} aria-label={intl.formatMessage(m.exVisualization)}>
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
            <Visualization explorer={explorer} question={question} answer={answer.data} />
          )}
        </article>
      </div>
    </section>
  )
}

function Visualization({ explorer, question, answer }: { explorer: Explorer; question: Question; answer: Answer }) {
  const intl = useIntl()
  const series = chartSeries(answer, explorer, question)
  const newest = newestComputed(answer)
  const title = (measure: string) => explorer.indicators.find((i) => i.measure === measure)?.title ?? measure
  const placeName = (code: string) => [explorer.place, ...explorer.places].find((p) => p?.code === code)?.name ?? code
  const facilityOnly = [...new Set(answer.cells.filter((c) => c.facilityOnly !== undefined).map((c) => c.indicator))]
  // Completeness is the same for every indicator a place's facilities sync: read it from the first that has figures.
  const counted = answer.cells.find((c) => c.facilityOnly === undefined)?.indicator
  const completeness = answer.cells.filter((c) => c.indicator === counted)
  const incomplete = completeness.filter((c) => (c.missing?.length ?? 0) > 0)
  const latest = answer.periods[answer.periods.length - 1]?.id
  return (
    <>
      <ColumnChart series={series} periods={answer.periods.map((p) => p.id)} label={intl.formatMessage(m.exChartLabel, { indicators: question.indicators.map(title).join(', '), from: question.from, to: question.to })} />
      <table className={css({ fontSize: 'sm', borderCollapse: 'collapse', '& th, & td': { textAlign: 'start', px: '2', py: '1', borderBottom: '1px solid', borderColor: 'border' } })}>
        <caption className={css({ textAlign: 'start', fontWeight: 'semibold' })}>{intl.formatMessage(m.exFigures)}</caption>
        <thead>
          <tr>
            <th scope="col">{intl.formatMessage(m.exSeries)}</th>
            {answer.periods.map((p) => (
              <th key={p.id} scope="col">
                {p.id}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {series.map((s) => (
            <tr key={`${s.indicator} ${s.place}`}>
              <th scope="row">{s.label}</th>
              {s.values.map((value, index) => (
                <td key={answer.periods[index]?.id}>{value === null ? '—' : intl.formatNumber(value)}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
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

/** A column chart: the periods along the bottom, a bar per series in each, a dash where there is no figure. */
function ColumnChart({ series, periods, label }: { series: Series[]; periods: string[]; label: string }) {
  const intl = useIntl()
  const width = 640
  const height = 240
  const left = 48
  const bottom = 36
  const values = series.flatMap((s) => s.values).filter((v): v is number => v !== null)
  const most = Math.max(1, ...values) * 1.1
  const column = (width - left - 8) / Math.max(1, periods.length)
  const bar = Math.max(3, (column * 0.8) / Math.max(1, series.length))
  const y = (value: number) => height - bottom - ((height - bottom - 8) * value) / most
  return (
    <figure className={css({ display: 'flex', flexDirection: 'column', gap: '2', m: '0' })}>
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={label} className={css({ width: '100%', height: 'auto' })}>
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
              <rect key={`${k}-${periods[i]}`} x={x} y={y(value)} width={bar - 1} height={height - bottom - y(value)} fill={SERIES_COLORS[k % SERIES_COLORS.length]}>
                <title>{`${s.label}, ${periods[i]}: ${intl.formatNumber(value)}`}</title>
              </rect>
            )
          }),
        )}
      </svg>
      {series.length > 1 ? (
        <figcaption className={css({ display: 'flex', gap: '3', flexWrap: 'wrap', fontSize: 'xs' })}>
          {series.map((s, k) => (
            <span key={`${s.indicator} ${s.place}`} className={cx(css({ display: 'inline-flex', gap: '1', alignItems: 'center' }))}>
              <span aria-hidden="true" style={{ width: 10, height: 10, background: SERIES_COLORS[k % SERIES_COLORS.length], display: 'inline-block' }} />
              {s.label}
            </span>
          ))}
        </figcaption>
      ) : null}
    </figure>
  )
}
