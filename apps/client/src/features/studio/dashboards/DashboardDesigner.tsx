import type { StudioDashboardPreview, StudioProblem } from '@health/api-client'
import { css, cx } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useId, useState } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { dashboardDesignerMessages as m, explorerMessages as em, studioMessages as sm } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import type { Transport } from '../../../platform/types'
import { type Explorer, sharedDisaggregations } from '../../explorer/api'
import { Visualization } from '../../explorer/ExplorerPage'
import { button, control } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { designsQuery } from '../designs'
import { refusalText } from '../wording'
import {
  FOLLOWS,
  LAYOUTS,
  MOST_PERIODS,
  PERIOD_TYPES,
  SERIES,
  TYPES,
  addItem,
  moveItem,
  previewAnswer,
  previewExplorer,
  previewShown,
  readDashboard,
  removeItem,
  summaryOf,
  toggleIndicator,
  updateItem,
  withDisaggregation,
  withItem,
  withPeriodType,
  withPeriods,
  withSettings,
  writeDashboard,
  type Dashboard,
  type Item,
} from './dashboard'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
const columns = css({ display: 'grid', gap: '4', alignItems: 'start', gridTemplateColumns: { base: '1fr', xl: 'minmax(0, 2fr) minmax(0, 3fr)' } })
const panel = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface', p: '3', display: 'flex', flexDirection: 'column', gap: '3', minWidth: '0' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const good = css({ color: 'success.text' })
const row = css({ display: 'flex', alignItems: 'center', gap: '2', flexWrap: 'wrap' })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm', minWidth: '0' })
const input = cx(control, css({ minWidth: '0' }))
const group = css({ border: 'none', p: '0', m: '0', display: 'flex', flexDirection: 'column', gap: '1', '& legend': { fontSize: 'sm', mb: '1' } })
const small = css({
  minHeight: 'control',
  minWidth: 'control',
  px: '2',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})
const linkButton = css({ all: 'unset', cursor: 'pointer', textDecoration: 'underline', textDecorationStyle: 'dotted', minHeight: 'control', _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' } })
const header = css({ all: 'unset', cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: '0.5', width: '100%', minHeight: 'control', _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' } })
const card = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '2', display: 'flex', flexDirection: 'column', gap: '2' })

/** `value` once it has stayed the same for `ms`. */
function useSettled<T>(value: T, ms = 400): T {
  const [settled, setSettled] = useState(value)
  const text = JSON.stringify(value)
  useEffect(() => {
    const timer = setTimeout(() => setSettled(JSON.parse(text) as T), ms)
    return () => clearTimeout(timer)
  }, [text, ms])
  return settled
}

const failure = (intl: IntlShape, error: unknown) =>
  error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail || String(error.status) : refusalText(intl, error.refusal)) : error instanceof Error ? error.message : String(error)

/** The preview: central answers the posted Dashboard over test patients, with publication's problems. */
async function previewDashboard(transport: Transport, id: string, body: { dashboard: unknown; path: string; asOf: string }, signal?: AbortSignal): Promise<StudioDashboardPreview> {
  const response = await transport.request({ method: 'POST', path: `/studio/packages/${encodeURIComponent(id)}/dashboards/preview`, body, ...(signal === undefined ? {} : { signal }) })
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse('previewStudioDashboard', response.status, response.body)
  if (response.status === 200) return value as StudioDashboardPreview
  const problem = value as StudioProblem | null
  throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
}

/** Today, as `2026-04-30`. */
const today = () => new Date().toISOString().slice(0, 10)

/**
 * The Dashboard designer (studio.md § Dashboards; analytics.md § Dashboards; ADR 0075; #879): a
 * standard Dashboard of a Reporting package draft. The outline on the left (its title, its
 * Visualizations as an accordion, one open at a time, each built like the explorer's: type,
 * indicators, the last *n* periods or a fixed range, disaggregation, series, layout and whose
 * places it follows), publication's problems beside it as the author edits, and the preview on
 * the right, every Visualization drawn by the explorer's own charts over the package's test
 * patients as of a day. Saving goes through the file editor's `If-Match` path; a 412 keeps
 * nothing and offers *Reload*.
 */
export function DashboardDesigner({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  const file = useQuery({ ...fileQuery(platform.transport, id, path), retry: false })
  if (draft.data === undefined || file.data === undefined) {
    return draft.isError || file.isError ? (
      <p role="alert" className={alert}>
        {intl.formatMessage(sm.sdFailed)}
      </p>
    ) : (
      <p role="status">{intl.formatMessage(m.bdLoading)}</p>
    )
  }
  const read = readDashboard(file.data.content)
  if ('refusal' in read)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.bdUnreadable, { reason: read.refusal })}
      </p>
    )
  return (
    <Designer
      key={`${file.data.revision}:${file.dataUpdatedAt}`}
      id={id}
      path={path}
      draft={draft.data}
      file={file.data}
      initial={read.dashboard}
      onReload={async () => {
        await Promise.all([draft.refetch(), file.refetch()])
      }}
    />
  )
}

/** The indicators the Studio holds, `url|version` each, to pick from. */
function useIndicators(): { canonical: string; title: string }[] {
  const platform = usePlatform()
  const designs = useQuery(designsQuery(platform.transport)).data?.designs ?? []
  return designs.filter((d) => d.kind === 'indicator' && typeof d.url === 'string').map((d) => ({ canonical: `${d.url ?? ''}|${d.version ?? ''}`, title: d.title }))
}

function Designer({ id, path, draft, file, initial, onReload }: { id: string; path: string; draft: StudioPackagePage; file: StudioFile; initial: Dashboard; onReload: () => Promise<void> }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { heading: useId(), outline: useId(), validation: useId(), preview: useId(), asOf: useId(), title: useId(), description: useId() }
  const [dashboard, setDashboard] = useState(initial)
  const [revision, setRevision] = useState(file.revision)
  const [saved, setSaved] = useState(() => JSON.stringify(writeDashboard(initial)))
  const [open, setOpen] = useState<number | null>(initial.items.length > 0 ? 0 : null)
  const [asOf, setAsOf] = useState(today)
  const editable = draft.editable
  const indicators = useIndicators()
  const written = writeDashboard(dashboard)
  const settled = useSettled({ dashboard: written, path, asOf })
  const preview = useQuery({
    queryKey: [...studioKey, 'dashboard-preview', id, settled] as const,
    queryFn: ({ signal }) => previewDashboard(platform.transport, id, settled, signal),
    enabled: /^\d{4}-\d{2}-\d{2}$/.test(settled.asOf),
    retry: false,
    placeholderData: (previous) => previous,
  })
  const save = useMutation({
    mutationFn: () => saveFile(platform.transport, id, path, `${JSON.stringify(written, null, 2)}\n`, revision),
    onSuccess: async (next) => {
      setRevision(next.revision)
      setSaved(JSON.stringify(written))
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
  const stale = save.error instanceof StudioError && save.error.status === 412
  const dirty = JSON.stringify(written) !== saved
  const problems = preview.data?.problems ?? []
  const explorer = preview.data === undefined ? undefined : previewExplorer(preview.data, intl.formatMessage(m.bdSample))
  const titleOf = (item: Item) => (item.title.trim() === '' ? intl.formatMessage(m.bdUntitled) : item.title)
  return (
    <section className={page} aria-labelledby={ids.heading}>
      <h1 id={ids.heading} className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
        {dashboard.title.trim() === '' ? path : dashboard.title}
      </h1>
      {editable ? null : (
        <p className={muted}>{draft.editRefusal === null || draft.editRefusal === undefined ? intl.formatMessage(m.bdReadOnly) : intl.formatMessage(sm.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}</p>
      )}
      <div className={columns}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '4', minWidth: '0' })}>
          <nav aria-labelledby={ids.outline} className={panel}>
            <h2 id={ids.outline} className={subheading}>
              {intl.formatMessage(m.bdOutline)}
            </h2>
            <label className={label} htmlFor={ids.title}>
              {intl.formatMessage(m.bdTitle)}
              <input id={ids.title} className={input} value={dashboard.title} readOnly={!editable} onChange={(e) => setDashboard(withSettings(dashboard, { title: e.target.value }))} />
            </label>
            <label className={label} htmlFor={ids.description}>
              {intl.formatMessage(m.bdDescription)}
              <textarea id={ids.description} className={input} rows={2} value={dashboard.description ?? ''} readOnly={!editable} onChange={(e) => setDashboard(withSettings(dashboard, { description: e.target.value }))} />
            </label>
            <p className={muted}>{intl.formatMessage(m.bdId, { id: dashboard.id, path })}</p>
            <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.bdItems)}</h3>
            {dashboard.items.length === 0 ? <p className={muted}>{intl.formatMessage(m.bdNoItems)}</p> : null}
            <ul className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
              {dashboard.items.map((item, at) => {
                const count = problems.filter((p) => p.item === at).length
                const expanded = open === at
                const summary = summaryOf(item)
                return (
                  <li key={at} className={card} data-open={expanded}>
                    <button type="button" className={header} aria-expanded={expanded} aria-controls={`${ids.outline}-i${at}`} onClick={() => setOpen(expanded ? null : at)}>
                      <span className={css({ fontWeight: 'semibold' })}>{titleOf(item)}</span>
                      <span className={muted}>
                        {intl.formatMessage(m.bdSummary, summary)}
                        {count === 0 ? null : <span className={alert}> · {intl.formatMessage(m.bdProblemCount, { count })}</span>}
                      </span>
                    </button>
                    {expanded ? (
                      <div id={`${ids.outline}-i${at}`} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
                        {editable ? (
                          <div className={row}>
                            <button type="button" className={small} disabled={at === 0} aria-label={intl.formatMessage(m.bdMoveUp, { title: titleOf(item) })} onClick={() => (setDashboard(moveItem(dashboard, at, -1)), setOpen(at - 1))}>
                              ↑
                            </button>
                            <button type="button" className={small} disabled={at === dashboard.items.length - 1} aria-label={intl.formatMessage(m.bdMoveDown, { title: titleOf(item) })} onClick={() => (setDashboard(moveItem(dashboard, at, 1)), setOpen(at + 1))}>
                              ↓
                            </button>
                            <button type="button" className={small} aria-label={intl.formatMessage(m.bdRemove, { title: titleOf(item) })} onClick={() => (setDashboard(removeItem(dashboard, at)), setOpen(null))}>
                              ✕
                            </button>
                          </div>
                        ) : null}
                        <ItemEditor item={item} editable={editable} choices={indicators} explorer={explorer} onChange={(next) => setDashboard(updateItem(dashboard, at, () => next))} />
                      </div>
                    ) : null}
                  </li>
                )
              })}
            </ul>
            {editable ? (
              <div className={row}>
                <button
                  type="button"
                  className={small}
                  onClick={() => {
                    setDashboard(addItem(dashboard, intl.formatMessage(m.bdNewItem), indicators[0]?.canonical))
                    setOpen(dashboard.items.length)
                  }}
                >
                  {intl.formatMessage(m.bdAddItem)}
                </button>
              </div>
            ) : null}
          </nav>
          <section aria-labelledby={ids.validation} className={panel}>
            <h2 id={ids.validation} className={subheading}>
              {intl.formatMessage(m.bdValidation)}
            </h2>
            {preview.isError ? (
              <p role="alert" className={alert}>
                {intl.formatMessage(m.bdCheckFailed, { reason: failure(intl, preview.error) })}
              </p>
            ) : preview.data === undefined ? (
              <p role="status">{intl.formatMessage(m.bdChecking)}</p>
            ) : problems.length === 0 ? (
              <p className={good}>{intl.formatMessage(m.bdNoProblems)}</p>
            ) : (
              <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', listStyle: 'disc', pl: '5' })}>
                {problems.map((p, n) => (
                  <li key={n} className={alert}>
                    {p.item === null || p.item === undefined ? (
                      p.message
                    ) : (
                      <button type="button" className={linkButton} onClick={() => setOpen(p.item ?? null)}>
                        {p.message}
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </section>
          {editable ? (
            <div className={row}>
              <button type="button" className={button} disabled={save.isPending || stale || !dirty} onClick={() => save.mutate()}>
                {intl.formatMessage(m.bdSave)}
              </button>
              {save.isSuccess && !stale && !dirty ? <span role="status">{intl.formatMessage(m.bdSaved)}</span> : null}
            </div>
          ) : null}
          {stale ? (
            <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
              <p className={alert}>{intl.formatMessage(m.bdStale)}</p>
              <button type="button" className={button} onClick={() => void onReload()}>
                {intl.formatMessage(m.bdReload)}
              </button>
            </div>
          ) : save.isError ? (
            <p role="alert" className={alert}>
              {intl.formatMessage(m.bdFailed, { reason: failure(intl, save.error) })}
            </p>
          ) : null}
        </div>
        <section aria-labelledby={ids.preview} className={panel}>
          <h2 id={ids.preview} className={subheading}>
            {intl.formatMessage(m.bdPreview)}
          </h2>
          <label className={label} htmlFor={ids.asOf}>
            {intl.formatMessage(m.bdAsOf)}
            <input id={ids.asOf} type="date" className={input} value={asOf} aria-describedby={`${ids.asOf}-hint`} onChange={(e) => setAsOf(e.target.value)} />
            <span id={`${ids.asOf}-hint`} className={muted}>
              {intl.formatMessage(m.bdAsOfHint)}
            </span>
          </label>
          {preview.isError ? (
            <p role="alert" className={alert}>
              {intl.formatMessage(m.bdPreviewFailed, { reason: failure(intl, preview.error) })}
            </p>
          ) : preview.data === undefined ? (
            <p role="status">{intl.formatMessage(m.bdComputing)}</p>
          ) : (
            <p className={muted}>{intl.formatMessage(m.bdPreviewHint, { patients: preview.data.patients, sources: preview.data.sources.join(', ') })}</p>
          )}
          {dashboard.items.map((item, at) => {
            const shown = preview.data?.items[at]
            return (
              <article key={at} aria-label={titleOf(item)} className={css({ display: 'flex', flexDirection: 'column', gap: '2', overflowX: 'auto', borderTop: '1px solid', borderColor: 'border', pt: '3' })}>
                <h3 className={css({ fontWeight: 'semibold' })}>
                  <button type="button" className={linkButton} onClick={() => setOpen(at)}>
                    {titleOf(item)}
                  </button>
                </h3>
                {shown === undefined || explorer === undefined ? null : shown.answer === null || shown.answer === undefined || shown.from === null || shown.from === undefined || shown.to === null || shown.to === undefined ? (
                  <p className={muted}>{intl.formatMessage(m.bdItemFailed, { reason: shown.error ?? '' })}</p>
                ) : (
                  <>
                    <p className={muted}>{intl.formatMessage(m.bdPeriodsShown, { from: shown.from, to: shown.to })}</p>
                    {item.type === 'map' ? <p className={muted}>{intl.formatMessage(m.bdMapAsTable)}</p> : null}
                    <Visualization explorer={explorer} shown={previewShown(item, shown.from, shown.to)} answer={previewAnswer(shown.answer)} before={shown.before === null || shown.before === undefined ? undefined : previewAnswer(shown.before)} footer={false} />
                  </>
                )}
              </article>
            )
          })}
        </section>
      </div>
    </section>
  )
}

/** One Visualization's setup, as the explorer's pickers set it. */
function ItemEditor({ item, editable, choices, explorer, onChange }: { item: Item; editable: boolean; choices: { canonical: string; title: string }[]; explorer: Explorer | undefined; onChange: (next: Item) => void }) {
  const intl = useIntl()
  const ids = { title: useId(), id: useId(), type: useId(), periodType: useId(), last: useId(), from: useId(), to: useId(), disaggregation: useId(), series: useId(), layout: useId(), follow: useId() }
  const periods = item.question.periods
  // Every indicator picked is listed, found in the Studio or not, so an unknown one can be unpicked.
  const listed = [...choices, ...item.question.indicators.filter((measure) => !choices.some((c) => c.canonical === measure)).map((measure) => ({ canonical: measure, title: measure }))]
  const offered = explorer === undefined ? [] : sharedDisaggregations(explorer, item.question.indicators)
  const disaggregations = item.question.disaggregation === undefined || offered.some((d) => d.name === item.question.disaggregation) ? offered : [...offered, { name: item.question.disaggregation, by: 'code' as const }]
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <div className={row}>
        <label className={label} htmlFor={ids.title}>
          {intl.formatMessage(m.bdItemTitle)}
          <input id={ids.title} className={input} value={item.title} readOnly={!editable} onChange={(e) => onChange(withItem(item, { title: e.target.value }))} />
        </label>
        <label className={label} htmlFor={ids.id}>
          {intl.formatMessage(m.bdItemId)}
          <input id={ids.id} className={input} value={item.id} readOnly={!editable} onChange={(e) => onChange(withItem(item, { id: e.target.value }))} />
        </label>
        <label className={label} htmlFor={ids.type}>
          {intl.formatMessage(em.exChartType)}
          <select id={ids.type} className={input} value={item.type} disabled={!editable} onChange={(e) => onChange(withItem(item, { type: e.target.value as Item['type'] }))}>
            {TYPES.map((t) => (
              <option key={t} value={t}>
                {intl.formatMessage(em[`exType_${t}`])}
              </option>
            ))}
          </select>
        </label>
      </div>
      <fieldset className={group}>
        <legend>{intl.formatMessage(m.bdIndicators)}</legend>
        {listed.length === 0 ? <p className={muted}>{intl.formatMessage(m.bdNoChoices)}</p> : null}
        {listed.map((c) => (
          <label key={c.canonical} className={css({ display: 'flex', gap: '2', alignItems: 'center', fontSize: 'sm' })}>
            <input type="checkbox" checked={item.question.indicators.includes(c.canonical)} disabled={!editable} onChange={(e) => onChange(toggleIndicator(item, c.canonical, e.target.checked))} />
            {c.title}
          </label>
        ))}
      </fieldset>
      <fieldset className={group}>
        <legend>{intl.formatMessage(m.bdPeriods)}</legend>
        <div className={row}>
          <label className={label} htmlFor={ids.periodType}>
            {intl.formatMessage(em.exPeriodType)}
            <select id={ids.periodType} className={input} value={item.question.periodType} disabled={!editable} onChange={(e) => onChange(withPeriodType(item, e.target.value as Item['question']['periodType']))}>
              {PERIOD_TYPES.map((t) => (
                <option key={t} value={t}>
                  {intl.formatMessage(em[`exPeriodType_${t}`])}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label className={css({ display: 'flex', gap: '2', alignItems: 'center', fontSize: 'sm' })}>
          <input type="radio" checked={'last' in periods} disabled={!editable} onChange={() => onChange(withPeriods(item, { last: 6 }))} />
          {intl.formatMessage(m.bdLast)}
        </label>
        {'last' in periods ? (
          <label className={label} htmlFor={ids.last}>
            {intl.formatMessage(m.bdLastCount)}
            <input id={ids.last} type="number" min={1} max={MOST_PERIODS} className={input} value={periods.last} readOnly={!editable} onChange={(e) => onChange(withPeriods(item, { last: Number(e.target.value) }))} />
          </label>
        ) : null}
        <label className={css({ display: 'flex', gap: '2', alignItems: 'center', fontSize: 'sm' })}>
          <input type="radio" checked={!('last' in periods)} disabled={!editable} onChange={() => onChange(withPeriods(item, { from: '', to: '' }))} />
          {intl.formatMessage(m.bdRange)}
        </label>
        {'last' in periods ? null : (
          <>
            <div className={row}>
              <label className={label} htmlFor={ids.from}>
                {intl.formatMessage(m.bdFrom)}
                <input id={ids.from} className={input} value={periods.from} readOnly={!editable} onChange={(e) => onChange(withPeriods(item, { ...periods, from: e.target.value }))} />
              </label>
              <label className={label} htmlFor={ids.to}>
                {intl.formatMessage(m.bdTo)}
                <input id={ids.to} className={input} value={periods.to} readOnly={!editable} onChange={(e) => onChange(withPeriods(item, { ...periods, to: e.target.value }))} />
              </label>
            </div>
            <p className={muted}>{intl.formatMessage(m.bdRangeHint)}</p>
          </>
        )}
      </fieldset>
      <div className={row}>
        <label className={label} htmlFor={ids.disaggregation}>
          {intl.formatMessage(em.exDisaggregation)}
          <select id={ids.disaggregation} className={input} value={item.question.disaggregation ?? ''} disabled={!editable} onChange={(e) => onChange(withDisaggregation(item, e.target.value))}>
            <option value="">{intl.formatMessage(em.exNoDisaggregation)}</option>
            {disaggregations.map((d) => (
              <option key={d.name} value={d.name}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
        <label className={label} htmlFor={ids.series}>
          {intl.formatMessage(em.exSeriesBy)}
          <select id={ids.series} className={input} value={item.series} disabled={!editable} onChange={(e) => onChange(withItem(item, { series: e.target.value as Item['series'] }))}>
            {SERIES.map((s) => (
              <option key={s} value={s} disabled={s === 'band' && item.question.disaggregation === undefined}>
                {intl.formatMessage(em[`exSeriesBy_${s}`])}
              </option>
            ))}
          </select>
        </label>
        {item.type === 'table' ? (
          <label className={label} htmlFor={ids.layout}>
            {intl.formatMessage(em.exLayout)}
            <select id={ids.layout} className={input} value={item.layout} disabled={!editable} onChange={(e) => onChange(withItem(item, { layout: e.target.value as Item['layout'] }))}>
              {LAYOUTS.map((l) => (
                <option key={l} value={l}>
                  {intl.formatMessage(em[`exLayout_${l}`])}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <label className={label} htmlFor={ids.follow}>
          {intl.formatMessage(m.bdFollow)}
          <select id={ids.follow} className={input} value={item.follow} disabled={!editable} onChange={(e) => onChange(withItem(item, { follow: e.target.value as Item['follow'] }))}>
            {FOLLOWS.map((f) => (
              <option key={f} value={f}>
                {intl.formatMessage(m[`bdFollow_${f}`])}
              </option>
            ))}
          </select>
        </label>
      </div>
    </div>
  )
}
