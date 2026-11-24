import { css } from '@health/design-system/css'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { type IntlShape, useIntl } from 'react-intl'

import { explorerMessages as m, reportMessages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import type { Cell } from '../studio/reports/layout'
import { type GridTexts, type ListedReport, type ReportRun, ReportsError, closedPeriods, reportCsv, reportsQuery, runGrids, runQuery } from './api'

const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const layout = css({ display: 'grid', gap: '4', gridTemplateColumns: { base: '1fr', md: '16rem 1fr' }, alignItems: 'start', _print: { display: 'block' } })
const panel = css({ display: 'flex', flexDirection: 'column', gap: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '3', _print: { display: 'none' } })
const field = css({ border: '1px solid', borderColor: 'border.control', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const button = css({ border: '1px solid', borderColor: 'border.control', borderRadius: 'md', px: 'controlX', minHeight: 'control', bg: 'bg.surface', cursor: 'pointer' })
const pressable = css({
  border: '1px solid',
  borderColor: 'border.control',
  borderRadius: 'md',
  px: 'controlX',
  minHeight: 'control',
  bg: 'bg.surface',
  cursor: 'pointer',
  textAlign: 'start',
  width: 'full',
  '&[aria-pressed=true]': { bg: 'bg.selected', color: 'fg.selected', borderColor: 'accent.default', fontWeight: 'semibold' },
})
const table = css({
  borderCollapse: 'collapse',
  width: 'full',
  fontSize: 'sm',
  '& th, & td': { border: '1px solid', borderColor: 'border', px: '2', py: '1', textAlign: 'start' },
  '& td': { textAlign: 'end', fontVariantNumeric: 'tabular-nums' },
})
const noPrint = css({ _print: { display: 'none' } })

/** The words the grid is written in, on screen and in the CSV alike. */
export function gridTexts(intl: IntlShape): GridTexts {
  return {
    indicator: intl.formatMessage(reportMessages.rdIndicatorColumn),
    total: intl.formatMessage(reportMessages.rdTotal),
    value: intl.formatMessage(reportMessages.rdValue),
    change: intl.formatMessage(reportMessages.rdChangeColumn),
    notShown: (cell: Cell) => intl.formatMessage(m.rpNotShown, { reason: (cell.notShown ?? 'no-preview').replace(/-([a-z])/g, (_, c: string) => c.toUpperCase()), name: cell.missing ?? '' }),
    figure: (value: string, calculation: string) => (calculation === 'percentage' ? `${value}%` : value),
  }
}

/**
 * Reports (analytics.md § Reports; ADR 0070, ADR 0072; #557): the Reports the session may run on
 * the left, with a place in its area and a closed period of the Report's type; the Report on the
 * right, read from stored Indicator Reports, with where and when its figures were computed and
 * the facilities missing (never counted as zero), to print or save as PDF, or export as CSV. On a
 * Facility Edge Node it runs for the facility, offline.
 */
export function ReportsPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const reports = useQuery(reportsQuery(transport))
  if (reports.data === undefined) {
    if (reports.error === null) return <p role="status">{intl.formatMessage(m.rpLoading)}</p>
    const forbidden = reports.error instanceof ReportsError && reports.error.status === 403
    return (
      <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        <h1 className={heading}>{intl.formatMessage(m.rpTitle)}</h1>
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(forbidden ? m.rpForbidden : m.rpFailed)}
        </p>
      </section>
    )
  }
  return <Reports reports={reports.data.reports} />
}

function Reports({ reports }: { reports: ListedReport[] }) {
  const intl = useIntl()
  const placeId = useId()
  const periodId = useId()
  const [picked, setPicked] = useState<string | undefined>(reports[0]?.id)
  const report = reports.find((r) => r.id === picked)
  const periods = report === undefined ? [] : closedPeriods(report.periodType, new Date(), 12)
  const [place, setPlace] = useState<string | undefined>(undefined)
  const [period, setPeriod] = useState<string | undefined>(undefined)
  const placeCode = report?.places.some((p) => p.code === place) ? place : report?.places[0]?.code
  const periodPicked = period !== undefined && periods.includes(period) ? period : periods[0]
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={`${heading} ${noPrint}`}>{intl.formatMessage(m.rpTitle)}</h1>
      {reports.length === 0 ? (
        <p className={muted}>{intl.formatMessage(m.rpNone)}</p>
      ) : (
        <div className={layout}>
          <div className={panel}>
            <ul aria-label={intl.formatMessage(m.rpTitle)} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
              {reports.map((r) => (
                <li key={r.id}>
                  <button type="button" className={pressable} aria-pressed={picked === r.id} onClick={() => setPicked(r.id)}>
                    {r.title}
                    <span className={css({ display: 'block', fontSize: 'xs', fontWeight: 'normal' })}>{intl.formatMessage(m.rpFrom, { package: r.package })}</span>
                  </button>
                </li>
              ))}
            </ul>
            {report === undefined ? null : (
              <>
                <label htmlFor={placeId}>{intl.formatMessage(m.rpPlace)}</label>
                <select id={placeId} className={field} value={placeCode} onChange={(e) => setPlace(e.target.value)}>
                  {report.places.map((p) => (
                    <option key={p.code} value={p.code}>
                      {p.name}
                    </option>
                  ))}
                </select>
                <label htmlFor={periodId}>{intl.formatMessage(m.rpPeriod)}</label>
                <select id={periodId} className={field} value={periodPicked} onChange={(e) => setPeriod(e.target.value)}>
                  {periods.map((p) => (
                    <option key={p} value={p}>
                      {p}
                    </option>
                  ))}
                </select>
              </>
            )}
          </div>
          {report === undefined || placeCode === undefined || periodPicked === undefined ? null : <Run report={report} place={placeCode} period={periodPicked} />}
        </div>
      )}
    </section>
  )
}

function Run({ report, place, period }: { report: ListedReport; place: string; period: string }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const run = useQuery({ ...runQuery(transport, { report: report.id, place, period }), placeholderData: keepPreviousData })
  if (run.data === undefined) {
    if (run.error === null) return <p role="status">{intl.formatMessage(m.rpRunning)}</p>
    const reason = run.error instanceof ReportsError ? (run.error.detail ?? String(run.error.status)) : run.error.message
    return (
      <p role="alert" className={css({ color: 'danger.text' })}>
        {intl.formatMessage(m.rpRunFailed, { reason })}
      </p>
    )
  }
  return <Shown report={report} run={run.data} />
}

function Shown({ report, run }: { report: ListedReport; run: ReportRun }) {
  const intl = useIntl()
  const texts = gridTexts(intl)
  const grids = runGrids(report.definition, run, texts)
  const download = () => {
    const blob = new Blob([reportCsv(report.definition, run, texts)], { type: 'text/csv;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `${report.definition.id}-${run.place.code}-${run.period.id}.csv`
    link.click()
    URL.revokeObjectURL(url)
  }
  const c = run.completeness
  return (
    <article aria-label={run.report.title} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <h2 className={css({ fontSize: 'xl', fontWeight: 'semibold' })}>{run.report.title}</h2>
      <p>{intl.formatMessage(m.rpWhere, { where: run.where, place: run.place.name, period: run.period.id, start: run.period.start, end: run.period.end })}</p>
      <p className={muted}>
        {intl.formatMessage(m.rpReported, { reported: c.reported, expected: c.expected })} {c.computedAt === null ? null : intl.formatMessage(m.rpComputedAt, { at: c.computedAt })}
      </p>
      {c.missing.length === 0 ? null : (
        <ul className={muted}>
          {c.missing.map((f) => (
            <li key={f.facility}>{f.lastSync === null ? intl.formatMessage(m.rpNeverSynced, { name: f.name }) : intl.formatMessage(m.rpMissing, { name: f.name, lastSync: f.lastSync })}</li>
          ))}
        </ul>
      )}
      <div className={`${noPrint} ${css({ display: 'flex', gap: '2' })}`}>
        <button type="button" className={button} onClick={() => window.print()}>
          {intl.formatMessage(m.rpPrint)}
        </button>
        <button type="button" className={button} onClick={download}>
          {intl.formatMessage(m.rpCsv)}
        </button>
      </div>
      {report.definition.sections.map((section, n) => {
        const shown = run.sections[n]
        if (section.kind === 'lineList' || shown?.kind !== 'table') {
          return (
            <section key={section.id} aria-label={section.title}>
              <h3 className={css({ fontWeight: 'semibold' })}>{section.title}</h3>
              <p className={muted}>{intl.formatMessage(run.where === 'central' ? m.rpLineListCentral : m.rpLineListLater)}</p>
            </section>
          )
        }
        const grid = grids.find((g) => g.section === section)?.grid
        return (
          <section key={section.id} aria-label={section.title} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
            <h3 className={css({ fontWeight: 'semibold' })}>{section.title}</h3>
            <table className={table}>
              <caption className={css({ srOnly: true })}>{section.title}</caption>
              <thead>
                <tr>
                  {grid?.header.map((h, i) => (
                    <th key={i} scope="col">
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {grid?.rows.map((row, i) => (
                  <tr key={i}>
                    <th scope="row">{row.head}</th>
                    {row.cells.map((cell, j) => (
                      <td key={j}>{cell}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
            {shown.rows.map((row) => {
              const label = section.rows.find((r) => r.id === row.row)?.label ?? row.row
              if (row.error !== null) return <p key={row.row} className={muted}>{intl.formatMessage(m.rpRowError, { label, reason: row.error })}</p>
              if (row.estimate === null || row.estimate === undefined) return null
              return (
                <p key={row.row} className={muted}>
                  {intl.formatMessage(m.rpEstimate, { label, title: row.estimate.title, value: row.estimate.value ?? '—' })}
                </p>
              )
            })}
          </section>
        )
      })}
    </article>
  )
}
