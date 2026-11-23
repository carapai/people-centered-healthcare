import type { StudioProblem, StudioReportPreview, StudioReportRow, StudioReportSection } from '@health/api-client'
import { css, cx } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useId, useState, type ReactNode } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { reportMessages as m, studioMessages as sm } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import type { Transport } from '../../../platform/types'
import { button, control } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { designsQuery, icu } from '../designs'
import { codeSystemsQuery, conceptsQuery } from '../terminology/api'
import { refusalText } from '../wording'
import { countedAt, layoutTable, listCellText, sentCells, shownAt, type At, type Cell } from './layout'
import {
  LEVELS,
  PERIOD_EXAMPLES,
  PERIOD_TYPES,
  PICKS,
  addAgeBands,
  addAttribute,
  addCoded,
  addConcept,
  addLineList,
  addRow,
  addTable,
  addVisitType,
  attributesFor,
  disaggregationName,
  moveColumn,
  moveRow,
  moveSection,
  readReport,
  removeColumn,
  removeDisaggregation,
  removeRow,
  removeSection,
  removeVisitType,
  summaryOf,
  toggleLevel,
  updateSection,
  withColumn,
  withDisaggregation,
  withList,
  withOptions,
  withRow,
  withRowPer,
  withSection,
  withSettings,
  withVisitType,
  writeReport,
  type Level,
  type LineListSection,
  type ListPick,
  type PeriodType,
  type Report,
  type TableSection,
} from './report'

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
const tag = css({ fontSize: 'xs', px: '1.5', py: '0.5', borderRadius: 'sm', bg: 'bg.subtle', border: '1px solid', borderColor: 'border' })
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
const table = css({ width: '100%', borderCollapse: 'collapse', fontSize: 'sm', '& th, & td': { textAlign: 'left', p: '1.5', borderBottom: '1px solid', borderColor: 'border', verticalAlign: 'top' } })
const cellButton = css({ all: 'unset', cursor: 'pointer', display: 'block', minWidth: 'control', minHeight: 'control', textDecoration: 'underline', textDecorationStyle: 'dotted', _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' } })
const header = css({
  all: 'unset',
  cursor: 'pointer',
  display: 'flex',
  flexDirection: 'column',
  gap: '0.5',
  width: '100%',
  minHeight: 'control',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
})
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

/** The preview: central computes the posted Report over test patients, with Validation's problems. */
async function previewReport(transport: Transport, id: string, body: { report: unknown; period: string }, signal?: AbortSignal): Promise<StudioReportPreview> {
  const response = await transport.request({ method: 'POST', path: `/studio/packages/${encodeURIComponent(id)}/reports/preview`, body, ...(signal === undefined ? {} : { signal }) })
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse('previewStudioReport', response.status, response.body)
  if (response.status === 200) return value as StudioReportPreview
  const problem = value as StudioProblem | null
  throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
}

/**
 * The Report designer (studio.md § Reports; ADR 0070, ADR 0072; #559): the outline on the left
 * (settings, the sections as an accordion, one open at a time, and Validation beside them), the
 * Report computed on the right for a period and a level over test patients, and the DHIS2
 * mapping sheet below. Saving goes through the file editor's `If-Match` path; a 412 keeps
 * nothing and offers *Reload*.
 */
export function ReportDesigner({ id, path }: { id: string; path: string }) {
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
      <p role="status">{intl.formatMessage(m.rdLoading)}</p>
    )
  }
  const read = readReport(file.data.content)
  if ('refusal' in read)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.rdUnreadable, { reason: read.refusal })}
      </p>
    )
  return (
    <Designer
      key={`${file.data.revision}:${file.dataUpdatedAt}`}
      id={id}
      path={path}
      draft={draft.data}
      file={file.data}
      initial={read.report}
      onReload={async () => {
        await Promise.all([draft.refetch(), file.refetch()])
      }}
    />
  )
}

/** What the pickers offer, from the Studio's designs. */
interface Choices {
  indicators: { canonical: string; title: string }[]
  ageBandSets: { canonical: string; title: string }[]
  programs: { url: string; title: string }[]
}

function useChoices(): Choices {
  const platform = usePlatform()
  const designs = useQuery(designsQuery(platform.transport)).data?.designs ?? []
  const pinned = (kind: string) => designs.filter((d) => d.kind === kind && typeof d.url === 'string').map((d) => ({ canonical: `${d.url ?? ''}|${d.version ?? ''}`, title: d.title }))
  return { indicators: pinned('indicator'), ageBandSets: pinned('age-band-set'), programs: designs.filter((d) => d.kind === 'program' && typeof d.url === 'string').map((d) => ({ url: d.url ?? '', title: d.title })) }
}

/** The cell the author last clicked in the preview. */
interface Selected {
  section: number
  row: number
  at: At[]
  total: boolean
}

function Designer({ id, path, draft, file, initial, onReload }: { id: string; path: string; draft: StudioPackagePage; file: StudioFile; initial: Report; onReload: () => Promise<void> }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { heading: useId(), outline: useId(), validation: useId(), preview: useId(), period: useId(), level: useId(), dhis2: useId() }
  const [report, setReport] = useState(initial)
  const [revision, setRevision] = useState(file.revision)
  const [saved, setSaved] = useState(() => JSON.stringify(writeReport(initial)))
  const [open, setOpen] = useState<number | null>(initial.sections.length > 0 ? 0 : null)
  const [selected, setSelected] = useState<Selected | null>(null)
  const [period, setPeriod] = useState(PERIOD_EXAMPLES[initial.periodType])
  const [level, setLevel] = useState<Level>(initial.levels[0] ?? 'facility')
  const editable = draft.editable
  const choices = useChoices()
  const written = writeReport(report)
  const settled = useSettled({ report: written, period })
  const preview = useQuery({
    queryKey: [...studioKey, 'report-preview', id, settled] as const,
    queryFn: ({ signal }) => previewReport(platform.transport, id, settled, signal),
    enabled: settled.period.trim() !== '',
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
  const shownLevel = report.levels.includes(level) ? level : (report.levels[0] ?? 'facility')
  const change = (next: Report) => setReport(next)
  const changeType = (periodType: PeriodType) => {
    setReport(withSettings(report, { periodType }))
    setPeriod(PERIOD_EXAMPLES[periodType])
  }
  const openCell = (cell: Selected) => {
    setOpen(cell.section)
    setSelected(cell)
  }
  return (
    <section className={page} aria-labelledby={ids.heading}>
      <h1 id={ids.heading} className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
        {report.title.trim() === '' ? path : report.title}
      </h1>
      {editable ? null : (
        <p className={muted}>{draft.editRefusal === null || draft.editRefusal === undefined ? intl.formatMessage(m.rdReadOnly) : intl.formatMessage(sm.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}</p>
      )}
      <div className={columns}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '4', minWidth: '0' })}>
          <nav aria-labelledby={ids.outline} className={panel}>
            <h2 id={ids.outline} className={subheading}>
              {intl.formatMessage(m.rdOutline)}
            </h2>
            <Settings report={report} editable={editable} path={path} onChange={change} onPeriodType={changeType} />
            <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.rdSections)}</h3>
            {report.sections.length === 0 ? <p className={muted}>{intl.formatMessage(m.rdNoSections)}</p> : null}
            <ul className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
              {report.sections.map((section, at) => {
                const count = problems.filter((p) => p.section === at).length
                const expanded = open === at
                const summary = summaryOf(section)
                return (
                  <li key={at} className={card} data-open={expanded}>
                    <button type="button" className={header} aria-expanded={expanded} aria-controls={`${ids.outline}-s${at}`} onClick={() => setOpen(expanded ? null : at)}>
                      <span className={css({ fontWeight: 'semibold' })}>{section.title.trim() === '' ? intl.formatMessage(m.rdUntitled) : section.title}</span>
                      <span className={muted}>
                        {summary.kind === 'table'
                          ? intl.formatMessage(m.rdTableSummary, { rows: summary.rows, by: summary.by, sent: String(summary.sent) })
                          : intl.formatMessage(m.rdListSummary, { rowPer: summary.rowPer, columns: summary.columns })}
                        {count === 0 ? null : <span className={alert}> · {intl.formatMessage(m.rdProblemCount, { count })}</span>}
                      </span>
                    </button>
                    {expanded ? (
                      <div id={`${ids.outline}-s${at}`} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
                        <SectionHead report={report} at={at} editable={editable} onChange={change} onMoved={setOpen} />
                        {section.kind === 'table' ? (
                          <TableEditor section={section} editable={editable} choices={choices} preview={preview.data?.sections[at]} onChange={(next) => change(updateSection(report, at, 'table', () => next))} />
                        ) : (
                          <LineListEditor id={id} section={section} editable={editable} choices={choices} onChange={(next) => change(updateSection(report, at, 'lineList', () => next))} />
                        )}
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
                    setReport(addTable(report, intl.formatMessage(m.rdNewTable)))
                    setOpen(report.sections.length)
                  }}
                >
                  {intl.formatMessage(m.rdAddTable)}
                </button>
                <button
                  type="button"
                  className={small}
                  onClick={() => {
                    setReport(addLineList(report, intl.formatMessage(m.rdNewList)))
                    setOpen(report.sections.length)
                  }}
                >
                  {intl.formatMessage(m.rdAddLineList)}
                </button>
              </div>
            ) : null}
          </nav>
          <Validation id={ids.validation} report={report} problems={problems} checking={preview.isFetching} failed={preview.isError ? failure(intl, preview.error) : null} onOpen={setOpen} />
          {editable ? (
            <div className={row}>
              <button type="button" className={button} disabled={save.isPending || stale || !dirty} onClick={() => save.mutate()}>
                {intl.formatMessage(m.rdSave)}
              </button>
              {save.isSuccess && !stale && !dirty ? <span role="status">{intl.formatMessage(m.rdSaved)}</span> : null}
            </div>
          ) : null}
          {stale ? (
            <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
              <p className={alert}>{intl.formatMessage(m.rdStale)}</p>
              <button type="button" className={button} onClick={() => void onReload()}>
                {intl.formatMessage(m.rdReload)}
              </button>
            </div>
          ) : save.isError ? (
            <p role="alert" className={alert}>
              {intl.formatMessage(m.rdFailed, { reason: failure(intl, save.error) })}
            </p>
          ) : null}
        </div>
        <section aria-labelledby={ids.preview} className={panel}>
          <h2 id={ids.preview} className={subheading}>
            {intl.formatMessage(m.rdPreview)}
          </h2>
          <div className={row}>
            <label className={label} htmlFor={ids.period}>
              {intl.formatMessage(m.rdPeriod)}
              <input id={ids.period} className={input} value={period} aria-describedby={`${ids.period}-hint`} onChange={(e) => setPeriod(e.target.value)} />
              <span id={`${ids.period}-hint`} className={muted}>
                {intl.formatMessage(m.rdPeriodExample, { example: PERIOD_EXAMPLES[report.periodType] })}
              </span>
            </label>
            <label className={label} htmlFor={ids.level}>
              {intl.formatMessage(m.rdLevelShown)}
              <select id={ids.level} className={input} value={shownLevel} onChange={(e) => setLevel(e.target.value as Level)}>
                {(report.levels.length === 0 ? (['facility'] as Level[]) : report.levels).map((l) => (
                  <option key={l} value={l}>
                    {intl.formatMessage(m.rdLevel, { level: l })}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {preview.isError ? (
            <p role="alert" className={alert}>
              {intl.formatMessage(m.rdPreviewFailed, { reason: failure(intl, preview.error) })}
            </p>
          ) : preview.data === undefined ? (
            <p role="status">{intl.formatMessage(m.rdComputing)}</p>
          ) : (
            <p className={muted}>
              {intl.formatMessage(m.rdPreviewHint, { patients: preview.data.patients, sources: preview.data.sources.join(', '), start: preview.data.period.start, end: preview.data.period.end })}
            </p>
          )}
          {report.sections.map((section, at) => (
            <div key={at} className={css({ display: 'flex', flexDirection: 'column', gap: '2', overflowX: 'auto' })}>
              <h3 className={css({ fontWeight: 'semibold' })}>
                <button type="button" className={cellButton} onClick={() => setOpen(at)}>
                  {section.title.trim() === '' ? intl.formatMessage(m.rdUntitled) : section.title}
                </button>
              </h3>
              {section.kind === 'table' ? (
                <PreviewTable section={section} at={at} preview={preview.data?.sections[at]} level={shownLevel} selected={selected} onCell={openCell} />
              ) : shownLevel === 'facility' ? (
                <PreviewList section={section} preview={preview.data?.sections[at]} />
              ) : (
                <p className={muted}>{intl.formatMessage(m.rdListFacility)}</p>
              )}
              {selected?.section === at && section.kind === 'table' ? (
                <CellDetail section={section} selected={selected} preview={preview.data?.sections[at]} level={shownLevel} previous={preview.data?.previous?.id ?? null} />
              ) : null}
            </div>
          ))}
        </section>
      </div>
      <Dhis2Sheet id={ids.dhis2} report={report} preview={preview.data} />
    </section>
  )
}

function Settings({ report, editable, path, onChange, onPeriodType }: { report: Report; editable: boolean; path: string; onChange: (next: Report) => void; onPeriodType: (type: PeriodType) => void }) {
  const intl = useIntl()
  const ids = { title: useId(), type: useId(), levels: useId() }
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <label className={label} htmlFor={ids.title}>
        {intl.formatMessage(m.rdTitle)}
        <input id={ids.title} className={input} value={report.title} readOnly={!editable} onChange={(e) => onChange(withSettings(report, { title: e.target.value }))} />
      </label>
      <p className={muted}>{intl.formatMessage(m.rdId, { id: report.id, path })}</p>
      <label className={label} htmlFor={ids.type}>
        {intl.formatMessage(m.rdPeriodType)}
        <select id={ids.type} className={input} value={report.periodType} disabled={!editable} onChange={(e) => onPeriodType(e.target.value as PeriodType)}>
          {PERIOD_TYPES.map((t) => (
            <option key={t} value={t}>
              {intl.formatMessage(m.rdPeriodTypeName, { type: t })}
            </option>
          ))}
        </select>
      </label>
      <fieldset className={css({ border: 'none', p: '0', m: '0', display: 'flex', gap: '3', flexWrap: 'wrap' })}>
        <legend className={css({ fontSize: 'sm', mb: '1' })}>{intl.formatMessage(m.rdLevels)}</legend>
        {LEVELS.map((l) => (
          <label key={l} className={row}>
            <input type="checkbox" checked={report.levels.includes(l)} disabled={!editable} onChange={(e) => onChange(toggleLevel(report, l, e.target.checked))} />
            {intl.formatMessage(m.rdLevel, { level: l })}
          </label>
        ))}
      </fieldset>
    </div>
  )
}

function SectionHead({ report, at, editable, onChange, onMoved }: { report: Report; at: number; editable: boolean; onChange: (next: Report) => void; onMoved: (at: number | null) => void }) {
  const intl = useIntl()
  const ids = { title: useId(), id: useId() }
  const section = report.sections[at]
  if (section === undefined) return null
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <div className={css({ display: 'grid', gap: '2', gridTemplateColumns: { base: '1fr', md: '2fr 1fr' } })}>
        <label className={label} htmlFor={ids.title}>
          {intl.formatMessage(m.rdSectionTitle)}
          <input id={ids.title} className={input} value={section.title} readOnly={!editable} onChange={(e) => onChange(withSection(report, at, { title: e.target.value }))} />
        </label>
        <label className={label} htmlFor={ids.id}>
          {intl.formatMessage(m.rdSectionId)}
          <input id={ids.id} className={cx(input, css({ fontFamily: 'mono' }))} value={section.id} readOnly={!editable} onChange={(e) => onChange(withSection(report, at, { id: e.target.value }))} />
        </label>
      </div>
      {editable ? (
        <div className={row}>
          <button
            type="button"
            className={small}
            disabled={at === 0}
            onClick={() => {
              onChange(moveSection(report, at, -1))
              onMoved(at - 1)
            }}
          >
            {intl.formatMessage(m.rdMoveSectionUp)}
          </button>
          <button
            type="button"
            className={small}
            disabled={at === report.sections.length - 1}
            onClick={() => {
              onChange(moveSection(report, at, 1))
              onMoved(at + 1)
            }}
          >
            {intl.formatMessage(m.rdMoveSectionDown)}
          </button>
          <button
            type="button"
            className={small}
            onClick={() => {
              onChange(removeSection(report, at))
              onMoved(null)
            }}
          >
            {intl.formatMessage(m.rdRemoveSection)}
          </button>
        </div>
      ) : null}
    </div>
  )
}

const titleOf = (choices: { canonical: string; title: string }[], canonical: string) => choices.find((c) => c.canonical === canonical)?.title ?? canonical

function TableEditor({ section, editable, choices, preview, onChange }: { section: TableSection; editable: boolean; choices: Choices; preview: StudioReportSection | undefined; onChange: (next: TableSection) => void }) {
  const intl = useIntl()
  const ids = { set: useId(), code: useId(), expression: useId(), indicator: useId() }
  const [set, setSet] = useState('')
  const [code, setCode] = useState('')
  const [expression, setExpression] = useState('')
  const [indicator, setIndicator] = useState('')
  const chosenSet = set === '' ? (choices.ageBandSets[0]?.canonical ?? '') : set
  const chosenIndicator = indicator === '' ? (choices.indicators[0]?.canonical ?? '') : indicator
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <fieldset className={css({ border: 'none', p: '0', m: '0', display: 'flex', flexDirection: 'column', gap: '2' })}>
        <legend className={css({ fontWeight: 'semibold', mb: '1' })}>{intl.formatMessage(m.rdDisaggregations)}</legend>
        {section.disaggregations.length === 0 ? <p className={muted}>{intl.formatMessage(m.rdNoDisaggregations)}</p> : null}
        {section.disaggregations.map((d, at) => {
          const name = 'ageBandSet' in d ? titleOf(choices.ageBandSets, d.ageBandSet) : d.code
          return (
            <div key={at} className={row}>
              <span className={css({ fontWeight: 'medium' })}>{name}</span>
              {'code' in d ? <code className={muted}>{d.expression}</code> : null}
              <label className={row}>
                <span className={muted}>{intl.formatMessage(m.rdAxis)}</span>
                <select className={input} aria-label={intl.formatMessage(m.rdAxisOf, { name })} value={d.axis} disabled={!editable} onChange={(e) => onChange(withDisaggregation(section, at, { ...d, axis: e.target.value === 'rows' ? 'rows' : 'columns' }))}>
                  <option value="rows">{intl.formatMessage(m.rdAxisName, { axis: 'rows' })}</option>
                  <option value="columns">{intl.formatMessage(m.rdAxisName, { axis: 'columns' })}</option>
                </select>
              </label>
              {editable ? (
                <button type="button" className={small} onClick={() => onChange(removeDisaggregation(section, at))}>
                  {intl.formatMessage(m.rdRemoveNamed, { name })}
                </button>
              ) : null}
            </div>
          )
        })}
        {editable ? (
          <>
            <div className={row}>
              <label className={label} htmlFor={ids.set}>
                {intl.formatMessage(m.rdSetToAdd)}
                <select id={ids.set} className={input} value={chosenSet} onChange={(e) => setSet(e.target.value)}>
                  {choices.ageBandSets.map((s) => (
                    <option key={s.canonical} value={s.canonical}>
                      {s.title} ({s.canonical})
                    </option>
                  ))}
                </select>
              </label>
              <button type="button" className={small} disabled={chosenSet === ''} onClick={() => onChange(addAgeBands(section, chosenSet))}>
                {intl.formatMessage(m.rdAddAgeBands)}
              </button>
            </div>
            <div className={row}>
              <label className={label} htmlFor={ids.code}>
                {intl.formatMessage(m.rdCode)}
                <input id={ids.code} className={input} value={code} onChange={(e) => setCode(e.target.value)} />
              </label>
              <label className={label} htmlFor={ids.expression}>
                {intl.formatMessage(m.rdExpression)}
                <input id={ids.expression} className={cx(input, css({ fontFamily: 'mono' }))} value={expression} onChange={(e) => setExpression(e.target.value)} />
              </label>
              <button
                type="button"
                className={small}
                disabled={code.trim() === '' || expression.trim() === ''}
                onClick={() => {
                  onChange(addCoded(section, code.trim(), expression.trim()))
                  setCode('')
                  setExpression('')
                }}
              >
                {intl.formatMessage(m.rdAddCoded)}
              </button>
            </div>
          </>
        ) : null}
      </fieldset>
      <fieldset className={css({ border: 'none', p: '0', m: '0', display: 'flex', flexDirection: 'column', gap: '1' })}>
        <legend className={css({ fontWeight: 'semibold', mb: '1' })}>{intl.formatMessage(m.rdOptions)}</legend>
        {(['columnTotals', 'rowTotals', 'change', 'sendToDhis2'] as const).map((option) => (
          <label key={option} className={row}>
            <input type="checkbox" checked={section[option]} disabled={!editable} onChange={(e) => onChange(withOptions(section, { [option]: e.target.checked }))} />
            {intl.formatMessage(m.rdOption, { option })}
          </label>
        ))}
      </fieldset>
      <fieldset className={css({ border: 'none', p: '0', m: '0', display: 'flex', flexDirection: 'column', gap: '2' })}>
        <legend className={css({ fontWeight: 'semibold', mb: '1' })}>{intl.formatMessage(m.rdIndicators)}</legend>
        {section.rows.map((r, at) => (
          <IndicatorRowEditor key={at} section={section} at={at} editable={editable} choices={choices} preview={preview?.rows.find((p) => p.row === r.id)} onChange={onChange} />
        ))}
        {editable ? (
          <div className={row}>
            <label className={label} htmlFor={ids.indicator}>
              {intl.formatMessage(m.rdIndicatorToAdd)}
              <select id={ids.indicator} className={input} value={chosenIndicator} onChange={(e) => setIndicator(e.target.value)}>
                {choices.indicators.map((i) => (
                  <option key={i.canonical} value={i.canonical}>
                    {i.title} ({i.canonical})
                  </option>
                ))}
              </select>
            </label>
            <button type="button" className={small} disabled={chosenIndicator === ''} onClick={() => onChange(addRow(section, chosenIndicator, titleOf(choices.indicators, chosenIndicator)))}>
              {intl.formatMessage(m.rdAddIndicator)}
            </button>
          </div>
        ) : null}
      </fieldset>
    </div>
  )
}

function IndicatorRowEditor({ section, at, editable, choices, preview, onChange }: { section: TableSection; at: number; editable: boolean; choices: Choices; preview: StudioReportRow | undefined; onChange: (next: TableSection) => void }) {
  const intl = useIntl()
  const ids = { label: useId(), id: useId(), indicator: useId() }
  const r = section.rows[at]
  if (r === undefined) return null
  const n = at + 1
  const options = choices.indicators.some((i) => i.canonical === r.indicator) ? choices.indicators : [{ canonical: r.indicator, title: r.indicator }, ...choices.indicators]
  const found = preview?.indicator
  const lacking = found === null || found === undefined ? [] : section.disaggregations.map(disaggregationName).filter((name) => preview?.current !== null && preview?.current !== undefined && !preview.current.disaggregations.some((d) => d.name === name))
  return (
    <div className={card}>
      <div className={css({ display: 'grid', gap: '2', gridTemplateColumns: { base: '1fr', md: '2fr 1fr' } })}>
        <label className={label} htmlFor={ids.label}>
          {intl.formatMessage(m.rdRowLabel, { at: n })}
          <input id={ids.label} className={input} value={r.label} readOnly={!editable} onChange={(e) => onChange(withRow(section, at, { label: e.target.value }))} />
        </label>
        <label className={label} htmlFor={ids.id}>
          {intl.formatMessage(m.rdRowId, { at: n })}
          <input id={ids.id} className={cx(input, css({ fontFamily: 'mono' }))} value={r.id} readOnly={!editable} onChange={(e) => onChange(withRow(section, at, { id: e.target.value }))} />
        </label>
      </div>
      <label className={label} htmlFor={ids.indicator}>
        {intl.formatMessage(m.rdRowIndicator, { at: n })}
        <select id={ids.indicator} className={input} value={r.indicator} disabled={!editable} onChange={(e) => onChange(withRow(section, at, { indicator: e.target.value }))}>
          {options.map((i) => (
            <option key={i.canonical} value={i.canonical}>
              {i.title} ({i.canonical})
            </option>
          ))}
        </select>
      </label>
      <div className={row}>
        <code className={muted}>{r.indicator}</code>
        {found === null || found === undefined ? (
          preview === undefined ? null : (
            <span className={cx(tag, alert)}>{intl.formatMessage(m.rdUnknownIndicator)}</span>
          )
        ) : (
          <>
            <span className={tag}>{intl.formatMessage(m.rdCalculation, { calculation: icu(found.calculation) })}</span>
            {shownAt(found.calculation, 'district') ? null : <span className={tag}>{intl.formatMessage(m.rdFacilityOnly)}</span>}
            {section.sendToDhis2 && found.calculation !== 'count' ? <span className={tag}>{intl.formatMessage(m.rdNotACount)}</span> : null}
            {found.published ? null : <span className={tag}>{intl.formatMessage(m.rdDraftOnly, { package: found.package })}</span>}
          </>
        )}
      </div>
      {lacking.map((name) => (
        <p key={name} className={muted}>
          {intl.formatMessage(m.rdNotBrokenDown, { name: titleOf(choices.ageBandSets, name) })}
        </p>
      ))}
      {editable ? (
        <div className={row}>
          <button type="button" className={small} disabled={at === 0} onClick={() => onChange(moveRow(section, at, -1))}>
            {intl.formatMessage(m.rdMoveUpNamed, { name: r.label })}
          </button>
          <button type="button" className={small} disabled={at === section.rows.length - 1} onClick={() => onChange(moveRow(section, at, 1))}>
            {intl.formatMessage(m.rdMoveDownNamed, { name: r.label })}
          </button>
          <button type="button" className={small} onClick={() => onChange(removeRow(section, at))}>
            {intl.formatMessage(m.rdRemoveNamed, { name: r.label })}
          </button>
        </div>
      ) : null}
    </div>
  )
}

/** A terminology search for a line list's concept column (#867): a code system the package is
 * authored against, searched on the server by code or name, a page at a time; using a concept
 * fills the column to add. */
function ConceptSearch({ id, onUse }: { id: string; onUse: (concept: { system: string; code: string; display: string }) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const ids = { heading: useId(), system: useId(), text: useId() }
  const systems = useQuery(codeSystemsQuery(platform.transport, id))
  const [chosen, setChosen] = useState('')
  const [text, setText] = useState('')
  const settled = useSettled(text.trim(), 300)
  const system = systems.data?.find((s) => `${s.url}|${s.version ?? ''}` === chosen) ?? systems.data?.[0]
  const ask = system === undefined ? null : { system: system.url, version: system.version ?? undefined, text: settled, count: 20 }
  const concepts = useQuery({ ...conceptsQuery(platform.transport, id, ask ?? { system: '' }), enabled: ask !== null && settled !== '' })
  return (
    <fieldset aria-labelledby={ids.heading} className={cx(card, css({ m: '0' }))}>
      <legend id={ids.heading} className={css({ fontSize: 'sm', fontWeight: 'semibold', px: '1' })}>
        {intl.formatMessage(m.rdConceptSearch)}
      </legend>
      {systems.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.rdConceptSearchFailed, { reason: failure(intl, systems.error) })}
        </p>
      ) : null}
      <div className={row}>
        <label className={label} htmlFor={ids.system}>
          {intl.formatMessage(m.rdConceptSystemToSearch)}
          <select id={ids.system} className={input} value={system === undefined ? '' : `${system.url}|${system.version ?? ''}`} onChange={(e) => setChosen(e.target.value)}>
            {(systems.data ?? []).map((s) => (
              <option key={`${s.url}|${s.version ?? ''}`} value={`${s.url}|${s.version ?? ''}`}>
                {s.title ?? s.url} ({s.package})
              </option>
            ))}
          </select>
        </label>
        <label className={label} htmlFor={ids.text}>
          {intl.formatMessage(m.rdConceptSearchText)}
          <input id={ids.text} type="search" className={input} value={text} onChange={(e) => setText(e.target.value)} />
        </label>
      </div>
      {settled === '' || ask === null ? null : concepts.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.rdConceptSearchFailed, { reason: failure(intl, concepts.error) })}
        </p>
      ) : concepts.data === undefined ? (
        <p role="status">{intl.formatMessage(m.rdConceptSearching)}</p>
      ) : (
        <>
          <p role="status" className={muted}>
            {concepts.data.total === 0
              ? intl.formatMessage(m.rdConceptNone)
              : intl.formatMessage(m.rdConceptFound, { total: concepts.data.total, shown: concepts.data.concepts.length < concepts.data.total ? concepts.data.concepts.length : 0 })}
          </p>
          <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
            {concepts.data.concepts.map((c) => (
              <li key={c.code}>
                <button type="button" className={small} onClick={() => onUse({ system: system?.url ?? '', code: c.code, display: c.display ?? c.code })}>
                  {intl.formatMessage(m.rdConceptUse, { code: c.code, display: c.display ?? '' })}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </fieldset>
  )
}

function LineListEditor({ id, section, editable, choices, onChange }: { id: string; section: LineListSection; editable: boolean; choices: Choices; onChange: (next: LineListSection) => void }) {
  const intl = useIntl()
  const ids = { rowPer: useId(), program: useId(), programs: useId(), enrollments: useId(), counted: useId(), attribute: useId(), system: useId(), code: useId(), conceptLabel: useId() }
  const attributes = attributesFor(section.rowPer)
  const [attribute, setAttribute] = useState<string>(attributes[0] ?? '')
  const [concept, setConcept] = useState({ system: '', code: '', label: '' })
  const chosenAttribute = attributes.includes(attribute as (typeof attributes)[number]) ? attribute : (attributes[0] ?? '')
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <label className={label} htmlFor={ids.rowPer}>
        {intl.formatMessage(m.rdRowPer)}
        <select id={ids.rowPer} className={input} value={section.rowPer} disabled={!editable} onChange={(e) => onChange(withRowPer(section, e.target.value === 'visit' ? 'visit' : 'enrollment'))}>
          <option value="enrollment">{intl.formatMessage(m.rdRowPerName, { rowPer: 'enrollment' })}</option>
          <option value="visit">{intl.formatMessage(m.rdRowPerName, { rowPer: 'visit' })}</option>
        </select>
      </label>
      <label className={label} htmlFor={ids.program}>
        {intl.formatMessage(m.rdProgram)}
        <input id={ids.program} className={cx(input, css({ fontFamily: 'mono' }))} list={ids.programs} value={section.program} readOnly={!editable} onChange={(e) => onChange(withList(section, { program: e.target.value }))} />
        <datalist id={ids.programs}>
          {choices.programs.map((p) => (
            <option key={p.url} value={p.url}>
              {p.title}
            </option>
          ))}
        </datalist>
      </label>
      {section.rowPer === 'enrollment' ? (
        <label className={label} htmlFor={ids.enrollments}>
          {intl.formatMessage(m.rdEnrollments)}
          <select id={ids.enrollments} className={input} value={section.enrollments ?? 'active'} disabled={!editable} onChange={(e) => onChange(withList(section, { enrollments: e.target.value === 'started' ? 'started' : 'active' }))}>
            <option value="started">{intl.formatMessage(m.rdEnrollmentsName, { which: 'started' })}</option>
            <option value="active">{intl.formatMessage(m.rdEnrollmentsName, { which: 'active' })}</option>
          </select>
        </label>
      ) : (
        <fieldset className={css({ border: 'none', p: '0', m: '0', display: 'flex', flexDirection: 'column', gap: '2' })}>
          <legend className={css({ fontSize: 'sm', mb: '1' })}>{intl.formatMessage(m.rdVisitTypes)}</legend>
          {(section.visitTypes ?? []).length === 0 ? <p className={muted}>{intl.formatMessage(m.rdAllVisits)}</p> : null}
          {(section.visitTypes ?? []).map((v, at) => (
            <div key={at} className={row}>
              <input className={cx(input, css({ fontFamily: 'mono' }))} aria-label={intl.formatMessage(m.rdVisitTypeSystem, { at: at + 1 })} value={v.system} readOnly={!editable} onChange={(e) => onChange(withVisitType(section, at, { ...v, system: e.target.value }))} />
              <input className={cx(input, css({ fontFamily: 'mono' }))} aria-label={intl.formatMessage(m.rdVisitTypeCode, { at: at + 1 })} value={v.code} readOnly={!editable} onChange={(e) => onChange(withVisitType(section, at, { ...v, code: e.target.value }))} />
              {editable ? (
                <button type="button" className={small} onClick={() => onChange(removeVisitType(section, at))}>
                  {intl.formatMessage(m.rdRemoveNamed, { name: v.code || String(at + 1) })}
                </button>
              ) : null}
            </div>
          ))}
          {editable ? (
            <div>
              <button type="button" className={small} onClick={() => onChange(addVisitType(section, { system: '', code: '' }))}>
                {intl.formatMessage(m.rdAddVisitType)}
              </button>
            </div>
          ) : null}
        </fieldset>
      )}
      <label className={label} htmlFor={ids.counted}>
        {intl.formatMessage(m.rdCountedBy)}
        <select id={ids.counted} className={input} value={section.countedBy ?? ''} disabled={!editable} onChange={(e) => onChange(withList(section, { countedBy: e.target.value }))}>
          <option value="">{intl.formatMessage(m.rdCountedByNone)}</option>
          {(section.countedBy === undefined || choices.indicators.some((i) => i.canonical === section.countedBy) ? choices.indicators : [{ canonical: section.countedBy, title: section.countedBy }, ...choices.indicators]).map((i) => (
            <option key={i.canonical} value={i.canonical}>
              {i.title} ({i.canonical})
            </option>
          ))}
        </select>
      </label>
      <fieldset className={css({ border: 'none', p: '0', m: '0', display: 'flex', flexDirection: 'column', gap: '2' })}>
        <legend className={css({ fontWeight: 'semibold', mb: '1' })}>{intl.formatMessage(m.rdColumns)}</legend>
        {section.columns.map((c, at) => (
          <div key={at} className={card}>
            <div className={row}>
              <input className={input} aria-label={intl.formatMessage(m.rdColumnLabel, { at: at + 1 })} value={c.label} readOnly={!editable} onChange={(e) => onChange(withColumn(section, at, { label: e.target.value }))} />
              <input className={cx(input, css({ fontFamily: 'mono' }))} aria-label={intl.formatMessage(m.rdColumnId, { at: at + 1 })} value={c.id} readOnly={!editable} onChange={(e) => onChange(withColumn(section, at, { id: e.target.value }))} />
            </div>
            {c.attribute !== undefined ? (
              <select className={input} aria-label={intl.formatMessage(m.rdColumnAttribute, { at: at + 1 })} value={c.attribute} disabled={!editable} onChange={(e) => onChange(withColumn(section, at, { attribute: e.target.value }))}>
                {(attributes.includes(c.attribute as (typeof attributes)[number]) ? attributes : [c.attribute, ...attributes]).map((a) => (
                  <option key={a} value={a}>
                    {a}
                  </option>
                ))}
              </select>
            ) : (
              <div className={row}>
                <input className={cx(input, css({ fontFamily: 'mono' }))} aria-label={intl.formatMessage(m.rdColumnSystem, { at: at + 1 })} value={c.concept?.system ?? ''} readOnly={!editable} onChange={(e) => onChange(withColumn(section, at, { concept: { system: e.target.value, code: c.concept?.code ?? '' } }))} />
                <input className={cx(input, css({ fontFamily: 'mono' }))} aria-label={intl.formatMessage(m.rdColumnCode, { at: at + 1 })} value={c.concept?.code ?? ''} readOnly={!editable} onChange={(e) => onChange(withColumn(section, at, { concept: { system: c.concept?.system ?? '', code: e.target.value } }))} />
                {section.rowPer === 'enrollment' ? (
                  <select className={input} aria-label={intl.formatMessage(m.rdColumnPick, { at: at + 1 })} value={c.pick ?? 'latest'} disabled={!editable} onChange={(e) => onChange(withColumn(section, at, { pick: e.target.value as ListPick }))}>
                    {PICKS.map((p) => (
                      <option key={p} value={p}>
                        {intl.formatMessage(m.rdPick, { pick: p })}
                      </option>
                    ))}
                  </select>
                ) : null}
              </div>
            )}
            {editable ? (
              <div className={row}>
                <button type="button" className={small} disabled={at === 0} onClick={() => onChange(moveColumn(section, at, -1))}>
                  {intl.formatMessage(m.rdMoveUpNamed, { name: c.label })}
                </button>
                <button type="button" className={small} disabled={at === section.columns.length - 1} onClick={() => onChange(moveColumn(section, at, 1))}>
                  {intl.formatMessage(m.rdMoveDownNamed, { name: c.label })}
                </button>
                <button type="button" className={small} onClick={() => onChange(removeColumn(section, at))}>
                  {intl.formatMessage(m.rdRemoveNamed, { name: c.label })}
                </button>
              </div>
            ) : null}
          </div>
        ))}
        {editable ? (
          <>
            <div className={row}>
              <label className={label} htmlFor={ids.attribute}>
                {intl.formatMessage(m.rdAttributeToAdd)}
                <select id={ids.attribute} className={input} value={chosenAttribute} onChange={(e) => setAttribute(e.target.value)}>
                  {attributes.map((a) => (
                    <option key={a} value={a}>
                      {a}
                    </option>
                  ))}
                </select>
              </label>
              <button type="button" className={small} onClick={() => onChange(addAttribute(section, chosenAttribute, chosenAttribute))}>
                {intl.formatMessage(m.rdAddAttribute)}
              </button>
            </div>
            <ConceptSearch id={id} onUse={(found) => setConcept({ system: found.system, code: found.code, label: concept.label.trim() === '' ? found.display : concept.label })} />
            <div className={row}>
              <label className={label} htmlFor={ids.conceptLabel}>
                {intl.formatMessage(m.rdConceptLabel)}
                <input id={ids.conceptLabel} className={input} value={concept.label} onChange={(e) => setConcept({ ...concept, label: e.target.value })} />
              </label>
              <label className={label} htmlFor={ids.system}>
                {intl.formatMessage(m.rdConceptSystem)}
                <input id={ids.system} className={cx(input, css({ fontFamily: 'mono' }))} value={concept.system} onChange={(e) => setConcept({ ...concept, system: e.target.value })} />
              </label>
              <label className={label} htmlFor={ids.code}>
                {intl.formatMessage(m.rdConceptCode)}
                <input id={ids.code} className={cx(input, css({ fontFamily: 'mono' }))} value={concept.code} onChange={(e) => setConcept({ ...concept, code: e.target.value })} />
              </label>
              <button
                type="button"
                className={small}
                disabled={concept.system.trim() === '' || concept.code.trim() === '' || concept.label.trim() === ''}
                onClick={() => {
                  onChange(addConcept(section, { system: concept.system.trim(), code: concept.code.trim() }, concept.label.trim()))
                  setConcept({ system: '', code: '', label: '' })
                }}
              >
                {intl.formatMessage(m.rdAddConcept)}
              </button>
            </div>
          </>
        ) : null}
      </fieldset>
    </div>
  )
}

function Validation({ id, report, problems, checking, failed, onOpen }: { id: string; report: Report; problems: StudioReportPreview['problems']; checking: boolean; failed: string | null; onOpen: (at: number) => void }) {
  const intl = useIntl()
  return (
    <section aria-labelledby={id} className={panel}>
      <h2 id={id} className={css({ fontWeight: 'semibold' })}>
        {intl.formatMessage(m.rdValidation)}
      </h2>
      {failed !== null ? (
        <p className={alert}>{failed}</p>
      ) : problems.length === 0 ? (
        <p className={good}>{checking ? intl.formatMessage(m.rdChecking) : intl.formatMessage(m.rdNoProblems)}</p>
      ) : (
        <>
          <p>{intl.formatMessage(m.rdProblemCount, { count: problems.length })}</p>
          <ul className={css({ pl: '5', listStyle: 'disc', display: 'flex', flexDirection: 'column', gap: '1' })}>
            {problems.map((p, n) => {
              const at = p.section
              const section = at === null || at === undefined ? undefined : report.sections[at]
              return (
                <li key={n} className={alert}>
                  {section === undefined || at === null || at === undefined ? (
                    p.message
                  ) : (
                    <button type="button" className={cellButton} onClick={() => onOpen(at)}>
                      {p.message}
                    </button>
                  )}
                </li>
              )
            })}
          </ul>
        </>
      )}
      <p className={muted}>{intl.formatMessage(m.rdAuthority)}</p>
    </section>
  )
}

const bandsText = (at: readonly At[]) => at.map((a) => a.label).join(' · ')

function figureText(intl: IntlShape, cell: Cell, calculation: string | undefined, choices?: Choices): ReactNode {
  if (cell.figure === null) {
    const reason = cell.notShown ?? 'no-preview'
    return <span title={intl.formatMessage(m.rdNotShown, { reason: icu(reason), name: cell.missing === undefined ? '' : choices === undefined ? cell.missing : titleOf(choices.ageBandSets, cell.missing) })}>{intl.formatMessage(m.rdNa)}</span>
  }
  const value = cell.figure.value
  return value === null ? (calculation === 'count' ? String(cell.figure.count) : '—') : intl.formatMessage(m.rdValueShown, { value, calculation: icu(calculation ?? 'count') })
}

function PreviewTable({ section, at, preview, level, selected, onCell }: { section: TableSection; at: number; preview: StudioReportSection | undefined; level: Level; selected: Selected | null; onCell: (cell: Selected) => void }) {
  const intl = useIntl()
  const layout = layoutTable(section, preview, level)
  return (
    <table className={table}>
      <caption className={css({ srOnly: true })}>{section.title}</caption>
      <thead>
        <tr>
          <th scope="col">{intl.formatMessage(m.rdIndicatorColumn)}</th>
          {layout.columns.map((c, n) => (
            <th key={n} scope="col">
              {c.total ? intl.formatMessage(m.rdTotal) : c.bands.length === 0 ? intl.formatMessage(m.rdValue) : bandsText(c.bands)}
            </th>
          ))}
          {layout.change ? <th scope="col">{intl.formatMessage(m.rdChangeColumn)}</th> : null}
        </tr>
      </thead>
      <tbody>
        {layout.rows.map((line, n) => {
          const found = preview?.rows.find((r) => r.row === line.row.id)
          const name = line.total ? `${line.row.label} · ${intl.formatMessage(m.rdTotal)}` : line.bands.length === 0 ? line.row.label : `${line.row.label} · ${bandsText(line.bands)}`
          return (
            <tr key={n}>
              <th scope="row">{name}</th>
              {line.cells.map((cell, c) => {
                const isSelected = selected?.section === at && selected.row === line.index && JSON.stringify(selected.at) === JSON.stringify(cell.at) && selected.total === cell.total
                return (
                  <td key={c} aria-current={isSelected ? 'true' : undefined}>
                    <button type="button" className={cellButton} onClick={() => onCell({ section: at, row: line.index, at: cell.at, total: cell.total })}>
                      {figureText(intl, cell, found?.indicator?.calculation)}
                    </button>
                  </td>
                )
              })}
              {line.change === null ? null : <td>{line.change.change === null ? '—' : `${line.change.change}%`}</td>}
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}

/** A line list's rows over the test patients, one per enrollment or visit (#867). */
function PreviewList({ section, preview }: { section: LineListSection; preview: StudioReportSection | undefined }) {
  const intl = useIntl()
  const list = preview?.list
  return (
    <>
      {list?.program === null || list?.program === undefined ? null : (
        <p className={muted}>
          {intl.formatMessage(m.rdListProgram, { program: list.program.title, episodeType: list.program.episodeType, package: list.program.package, published: String(list.program.published) })}
        </p>
      )}
      <table className={table}>
        <caption className={css({ srOnly: true })}>{section.title}</caption>
        <thead>
          <tr>
            <th scope="col">{intl.formatMessage(m.rdListPatient)}</th>
            {section.columns.map((c, n) => (
              <th key={n} scope="col">
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {(list?.rows ?? []).map((r) => (
            <tr key={r.record}>
              <th scope="row">{r.patient}</th>
              {section.columns.map((c, n) => (
                <td key={n}>{listCellText(r.cells[n] ?? [], c.pick === 'all')}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {list === null || list === undefined ? null : list.error !== null && list.error !== undefined ? (
        <p className={alert}>{intl.formatMessage(m.rdListFailed, { reason: list.error })}</p>
      ) : list.rows.length === 0 ? (
        <p className={muted}>{intl.formatMessage(m.rdListEmpty)}</p>
      ) : (
        <p className={muted}>{intl.formatMessage(m.rdListCount, { rows: list.rows.length, rowPer: section.rowPer })}</p>
      )}
    </>
  )
}

function CellDetail({ section, selected, preview, level, previous }: { section: TableSection; selected: Selected; preview: StudioReportSection | undefined; level: Level; previous: string | null }) {
  const intl = useIntl()
  const choices = useChoices()
  const r = section.rows[selected.row]
  if (r === undefined) return null
  const found = preview?.rows.find((p) => p.row === r.id)
  const layout = layoutTable(section, preview, level)
  const cell = layout.rows.flatMap((line) => (line.index === selected.row ? line.cells : [])).find((c) => JSON.stringify(c.at) === JSON.stringify(selected.at) && c.total === selected.total)
  const calculation = found?.indicator?.calculation
  const counted = countedAt(found, selected.at)
  return (
    <div role="region" aria-label={intl.formatMessage(m.rdCell)} className={card}>
      <p className={css({ fontWeight: 'semibold' })}>{selected.at.length === 0 ? r.label : `${r.label} · ${bandsText(selected.at)}`}</p>
      {cell === undefined ? null : (
        <>
          <p>{figureText(intl, cell, calculation, choices)}</p>
          {cell.figure === null ? (
            <p className={muted}>{intl.formatMessage(m.rdNotShown, { reason: icu(cell.notShown ?? 'no-preview'), name: cell.missing === undefined ? '' : titleOf(choices.ageBandSets, cell.missing) })}</p>
          ) : (
            <>
              <p>{cell.figure.denominator === null ? intl.formatMessage(m.rdCellCount, { count: cell.figure.count }) : intl.formatMessage(m.rdCellParts, { count: cell.figure.count, denominator: cell.figure.denominator })}</p>
              {cell.previous === null || previous === null ? null : <p>{intl.formatMessage(m.rdCellPrevious, { period: previous, value: cell.previous.value ?? String(cell.previous.count) })}</p>}
              <p>{counted.length === 0 ? intl.formatMessage(m.rdCellNobody) : intl.formatMessage(m.rdCellCounted, { names: counted.join(', ') })}</p>
            </>
          )}
        </>
      )}
      {section.sendToDhis2 ? <p className={muted}>{intl.formatMessage(m.rdCellMapping)}</p> : null}
    </div>
  )
}

function Dhis2Sheet({ id, report, preview }: { id: string; report: Report; preview: StudioReportPreview | undefined }) {
  const intl = useIntl()
  const cells = sentCells(report.sections.flatMap((section, index) => (section.kind === 'table' ? [{ section, index, preview: preview?.sections[index] }] : [])))
  return (
    <section aria-labelledby={id} className={panel}>
      <h2 id={id} className={subheading}>
        {intl.formatMessage(m.rdDhis2)}
      </h2>
      <p className={muted}>{intl.formatMessage(m.rdDhis2Hint)}</p>
      {cells.length === 0 ? (
        <p>{intl.formatMessage(m.rdDhis2None)}</p>
      ) : (
        <div className={css({ overflowX: 'auto' })}>
          <table className={table}>
            <caption className={css({ srOnly: true })}>{intl.formatMessage(m.rdDhis2)}</caption>
            <thead>
              <tr>
                <th scope="col">{intl.formatMessage(m.rdDhis2Section)}</th>
                <th scope="col">{intl.formatMessage(m.rdDhis2Row)}</th>
                <th scope="col">{intl.formatMessage(m.rdDhis2Bands)}</th>
                <th scope="col">{intl.formatMessage(m.rdDhis2Element)}</th>
                <th scope="col">{intl.formatMessage(m.rdDhis2Combo)}</th>
              </tr>
            </thead>
            <tbody>
              {cells.map((c, n) => (
                <tr key={n}>
                  <td>{c.sectionTitle}</td>
                  <td>{c.row.label}</td>
                  <td>
                    {c.total ? intl.formatMessage(m.rdTotal) : c.at.length === 0 ? '—' : bandsText(c.at)}
                    {c.total || c.notRecorded ? <span className={muted}> {intl.formatMessage(m.rdSentIfMapped)}</span> : null}
                  </td>
                  <td className={muted}>{intl.formatMessage(m.rdNotMapped)}</td>
                  <td className={muted}>{intl.formatMessage(m.rdNotMapped)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}
