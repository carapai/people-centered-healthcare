import type { StudioHeld, StudioIndicatorMapping, StudioIndicatorPatient, StudioIndicatorTry, StudioProblem } from '@health/api-client'
import { css, cx } from '@health/design-system/css'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useEffect, useId, useState, type ReactNode } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { indicatorMessages as m, studioMessages as sm } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import type { Transport } from '../../../platform/types'
import { button, control } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { designsQuery, editorOf, icu } from '../designs'
import { refusalText } from '../wording'
import { JOINS, type Group, type Join, type LiteralKind, type Operator, blank, kindOf, literal, operatorsFor, shown } from '../workflows/decisions'
import { DATE_UNITS, SIDES, compileCriteria, type Anchor, type Atom, type Criteria, type CriteriaNode, type DateCondition, type DateOp, type DateUnit, type Side } from './conditions'
import {
  BASES,
  BETWEEN_UNITS,
  CALCULATIONS,
  CONDITION_INPUTS,
  COUNTED,
  PERIOD_TYPES,
  RATE_FACTORS,
  addDenominator,
  declaredDates,
  estimateTypesOf,
  explainedConditions,
  hasDenominator,
  indicatorProblems,
  indicatorTest,
  periodOf,
  readIndicator,
  refersOf,
  removeDenominator,
  takesValue,
  withCalculation,
  withDenominatorEstimate,
  writeIndicator,
  type BetweenUnit,
  type Calculation,
  type Counted,
  type EstimateType,
  type Explained,
  type Indicator,
  type IndicatorProblem,
  type Other,
  type PeriodType,
} from './indicator'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
const columns = css({ display: 'grid', gap: '4', alignItems: 'start', gridTemplateColumns: { base: '1fr', xl: 'minmax(0, 3fr) minmax(0, 2fr)' } })
const panel = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface', p: '3', display: 'flex', flexDirection: 'column', gap: '3', minWidth: '0' })
const sticky = css({ position: { xl: 'sticky' }, top: { xl: '4' } })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const good = css({ color: 'success.text' })
const row = css({ display: 'flex', alignItems: 'center', gap: '2', flexWrap: 'wrap' })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm', minWidth: '0' })
const grid = css({ display: 'grid', gap: '3', gridTemplateColumns: { base: '1fr', md: 'repeat(2, minmax(0, 1fr))' } })
const input = cx(control, css({ minWidth: '0' }))
const mono = css({ fontFamily: 'mono', fontSize: 'xs', bg: 'bg.subtle', p: '2', borderRadius: 'sm', overflowWrap: 'anywhere' })
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
const table = css({ width: '100%', borderCollapse: 'collapse', fontSize: 'sm', '& th, & td': { textAlign: 'left', p: '1.5', borderBottom: '1px solid', borderColor: 'border' } })
const figure = css({ fontSize: '3xl', fontWeight: 'semibold' })

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

/** *Try*: the server evaluates the posted Measure over the package's test patients. */
async function tryIndicator(transport: Transport, id: string, body: { measure: unknown; periodType: PeriodType; period: string; conditions: string[] }, signal?: AbortSignal): Promise<StudioIndicatorTry> {
  const response = await transport.request({ method: 'POST', path: `/studio/packages/${encodeURIComponent(id)}/indicators/try`, body, ...(signal === undefined ? {} : { signal }) })
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse('tryStudioIndicator', response.status, response.body)
  if (response.status === 200) return value as StudioIndicatorTry
  const problem = value as StudioProblem | null
  throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
}

/** Where Reports lay out the indicator `url` (its HMIS mapping, #864), in each package's newest
 * published version and its open draft. */
async function indicatorMappings(transport: Transport, url: string, signal?: AbortSignal): Promise<StudioIndicatorMapping[]> {
  const response = await transport.request({ method: 'GET', path: `/studio/indicator-mappings?url=${encodeURIComponent(url)}`, ...(signal === undefined ? {} : { signal }) })
  const { parseResponse } = await import('@health/api-client')
  const value = parseResponse('listStudioIndicatorMappings', response.status, response.body)
  if (response.status === 200) return value as StudioIndicatorMapping[]
  const problem = value as StudioProblem | null
  throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
}

/**
 * The indicator editor (studio.md § Indicators; analytics.md; ADR 0068; #537): the definition on
 * the left (name, the period date, what it counts and calculates, the numerator, the denominator
 * behind *+ Add denominator*, and the disaggregations) and *Try* on the right, always visible,
 * where central evaluates the unsaved Measure over the package's test patients for a period.
 * Saving goes through the file editor's `If-Match` path; a 412 keeps nothing and offers *Reload*.
 */
export function IndicatorEditor({ id, path }: { id: string; path: string }) {
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
      <p role="status">{intl.formatMessage(m.inLoading)}</p>
    )
  }
  const read = readIndicator(file.data.content)
  if ('refusal' in read)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.inUnreadable, { reason: read.refusal })}
      </p>
    )
  return (
    <Editor
      key={`${file.data.revision}:${file.dataUpdatedAt}`}
      id={id}
      path={path}
      draft={draft.data}
      file={file.data}
      measure={read.measure}
      initial={read.indicator}
      onReload={async () => {
        await Promise.all([draft.refetch(), file.refetch()])
      }}
    />
  )
}

interface EditorProps {
  id: string
  path: string
  draft: StudioPackagePage
  file: StudioFile
  measure: Record<string, unknown>
  initial: Indicator
  onReload: () => Promise<void>
}

/** The package's other indicators, each with what it refers to, read from their files. */
function useOthers(id: string, draft: StudioPackagePage, path: string): Other[] {
  const platform = usePlatform()
  const files = draft.files.filter((f) => f.resourceType === 'Measure' && f.path !== path && typeof f.url === 'string')
  const read = useQueries({ queries: files.map((f) => ({ ...fileQuery(platform.transport, id, f.path), retry: false })) })
  return files.map((f, n) => {
    const content = read[n]?.data?.content
    let title = f.path
    if (content !== undefined) {
      try {
        const parsed = JSON.parse(content) as { title?: unknown }
        if (typeof parsed.title === 'string') title = parsed.title
      } catch {
        // The path stands in for an unreadable file's title.
      }
    }
    return { canonical: `${f.url ?? ''}|${f.version ?? ''}`, title, refers: content === undefined ? [] : refersOf(content) }
  })
}

function Editor({ id, path, draft, file, measure, initial, onReload }: EditorProps) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { heading: useId(), definition: useId(), problems: useId() }
  const [indicator, setIndicator] = useState(initial)
  const [revision, setRevision] = useState(file.revision)
  const [saved, setSaved] = useState(() => JSON.stringify(writeIndicator(measure, initial)))
  const [files, setFiles] = useState(() => draft.files.map((f) => f.path))
  const editable = draft.editable
  const others = useOthers(id, draft, path)
  const designs = useQuery(designsQuery(platform.transport))
  const ageBandSets = (designs.data?.designs ?? []).filter((d) => d.kind === 'age-band-set' && typeof d.url === 'string').map((d) => ({ canonical: `${d.url ?? ''}|${d.version ?? ''}`, title: d.title }))
  const estimateTypes = estimateTypesOf(designs.data?.designs ?? [])
  const self = `${typeof measure['url'] === 'string' ? measure['url'] : ''}|${typeof measure['version'] === 'string' ? measure['version'] : ''}`
  const problems = indicatorProblems(indicator, self, others)
  const written = writeIndicator(measure, indicator)
  const save = useMutation({
    mutationFn: () => saveFile(platform.transport, id, path, JSON.stringify(written, null, 2), revision),
    onSuccess: async (next) => {
      setRevision(next.revision)
      setSaved(JSON.stringify(written))
      setFiles(next.files.map((f) => f.path))
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
  const dirty = JSON.stringify(written) !== saved
  const saveTest = useMutation({
    mutationFn: async (tried: { periodType: PeriodType; period: string; result: StudioIndicatorTry }) => {
      const made = indicatorTest(self, indicator.title, tried.periodType, tried.period, tried.result, files)
      const next = await saveFile(platform.transport, id, made.path, `${JSON.stringify(made.test, null, 2)}\n`, revision)
      return { next, path: made.path }
    },
    onSuccess: async ({ next }) => {
      setRevision(next.revision)
      setFiles(next.files.map((f) => f.path))
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
  const stale = (save.error instanceof StudioError && save.error.status === 412) || (saveTest.error instanceof StudioError && saveTest.error.status === 412)
  const change = (next: Partial<Indicator>) => setIndicator((i) => ({ ...i, ...next }))
  const context: TreeContext = { editable, dates: declaredDates(indicator.basis), others }
  return (
    <section className={page} aria-labelledby={ids.heading}>
      <h1 id={ids.heading} className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
        {indicator.title.trim() === '' ? path : indicator.title}
      </h1>
      {editable ? null : (
        <p className={muted}>{draft.editRefusal === null || draft.editRefusal === undefined ? intl.formatMessage(m.inReadOnly) : intl.formatMessage(sm.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}</p>
      )}
      <div role="region" aria-labelledby={ids.problems} className={panel}>
        <h2 id={ids.problems} className={css({ fontWeight: 'semibold' })}>
          {problems.length === 0 ? intl.formatMessage(m.inNoProblems) : intl.formatMessage(m.inProblems, { count: problems.length })}
        </h2>
        {problems.length === 0 ? null : (
          <ul className={css({ pl: '5', listStyle: 'disc' })}>
            {problems.map((p, n) => (
              <li key={n} className={alert}>
                {problemText(intl, p, others)}
              </li>
            ))}
          </ul>
        )}
        <p className={muted}>{intl.formatMessage(m.inAuthority)}</p>
      </div>
      <div className={columns}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '4', minWidth: '0' })} role="region" aria-labelledby={ids.definition}>
          <h2 id={ids.definition} className={subheading}>
            {intl.formatMessage(m.inDefinition)}
          </h2>
          <Basics indicator={indicator} editable={editable} onChange={change} />
          <Calculates indicator={indicator} editable={editable} onChange={setIndicator} />
          <SideEditor
            title={intl.formatMessage(takesValue(indicator.calculation) ? m.inCounted : hasDenominator(indicator.calculation) ? m.inNumerator : m.inCounted)}
            hint={intl.formatMessage(hasDenominator(indicator.calculation) ? m.inNumeratorHint : m.inCountedHint)}
            tree={indicator.numerator}
            context={context}
            onChange={(numerator) => change({ numerator })}
          >
            {editable && !hasDenominator(indicator.calculation) && !takesValue(indicator.calculation) ? (
              <button type="button" className={small} onClick={() => setIndicator(addDenominator)}>
                {intl.formatMessage(m.inAddDenominator)}
              </button>
            ) : null}
          </SideEditor>
          {indicator.denominator === null ? null : indicator.denominatorEstimate === null ? (
            <SideEditor title={intl.formatMessage(m.inDenominator)} hint={intl.formatMessage(m.inDenominatorHint)} tree={indicator.denominator} context={context} onChange={(denominator) => change({ denominator })}>
              <DenominatorFrom indicator={indicator} editable={editable} types={estimateTypes} onChange={setIndicator} />
            </SideEditor>
          ) : (
            <EstimateDenominator indicator={indicator} editable={editable} types={estimateTypes} onChange={setIndicator} />
          )}
          <Disaggregations indicator={indicator} editable={editable} sets={ageBandSets} onChange={change} />
          <HmisMapping url={typeof measure['url'] === 'string' ? measure['url'] : ''} />
          {editable ? (
            <div className={row}>
              <button type="button" className={button} disabled={save.isPending || stale} onClick={() => save.mutate()}>
                {intl.formatMessage(m.inSave)}
              </button>
              {save.isSuccess && !stale ? <span role="status">{intl.formatMessage(m.inSaved)}</span> : null}
            </div>
          ) : null}
          {stale ? (
            <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
              <p className={alert}>{intl.formatMessage(m.inStale)}</p>
              <button type="button" className={button} onClick={() => void onReload()}>
                {intl.formatMessage(m.inReload)}
              </button>
            </div>
          ) : save.isError ? (
            <p role="alert" className={alert}>
              {intl.formatMessage(m.inFailed, { reason: failure(intl, save.error) })}
            </p>
          ) : null}
        </div>
        <TryPanel
          id={id}
          measure={written}
          explained={explainedConditions(indicator)}
          calculation={indicator.calculation}
          test={{
            allowed: editable && !dirty && !stale,
            why: editable ? (dirty ? intl.formatMessage(m.inTestUnsaved) : '') : intl.formatMessage(m.inReadOnly),
            pending: saveTest.isPending,
            saved: saveTest.isSuccess ? saveTest.data.path : null,
            failed: saveTest.isError && !stale ? failure(intl, saveTest.error) : null,
            onSave: (tried) => saveTest.mutate(tried),
          }}
        />
      </div>
    </section>
  )
}

function problemText(intl: IntlShape, problem: IndicatorProblem, others: readonly Other[]): string {
  const name = (canonical: string) => others.find((o) => o.canonical === canonical)?.title ?? canonical
  switch (problem.kind) {
    case 'no-title':
      return intl.formatMessage(m.inNoTitle)
    case 'no-period-date':
      return intl.formatMessage(m.inNoPeriodDate)
    case 'no-value':
      return intl.formatMessage(m.inNoValue)
    case 'unpinned':
      return intl.formatMessage(m.inUnpinned, { measure: problem.measure })
    case 'itself':
      return intl.formatMessage(m.inItself)
    case 'unknown':
      return intl.formatMessage(m.inUnknown, { measure: problem.measure })
    case 'circle':
      return intl.formatMessage(m.inCircle, { through: problem.through.map(name).join(' → ') })
    case 'no-disaggregation-name':
      return intl.formatMessage(m.inNoDisaggregationName, { at: problem.at + 1 })
    case 'twice':
      return intl.formatMessage(m.inTwice, { name: problem.name })
  }
}

// ---------------------------------------------------------------------------------------------
// The definition

function Basics({ indicator, editable, onChange }: { indicator: Indicator; editable: boolean; onChange: (next: Partial<Indicator>) => void }) {
  const intl = useIntl()
  const ids = { title: useId(), description: useId(), basis: useId(), periodDate: useId(), dates: useId(), counts: useId() }
  const dates = declaredDates(indicator.basis)
  return (
    <div className={panel}>
      <div className={grid}>
        <label className={label} htmlFor={ids.title}>
          {intl.formatMessage(m.inTitle)}
          <input id={ids.title} className={input} value={indicator.title} readOnly={!editable} onChange={(e) => onChange({ title: e.target.value })} />
        </label>
        <label className={label} htmlFor={ids.counts}>
          {intl.formatMessage(m.inCounts)}
          <select id={ids.counts} className={input} value={indicator.counts} disabled={!editable} onChange={(e) => onChange({ counts: e.target.value as Counted })}>
            {COUNTED.map((c) => (
              <option key={c} value={c}>
                {intl.formatMessage(m.inCountsName, { counts: c })}
              </option>
            ))}
          </select>
        </label>
        <label className={label} htmlFor={ids.basis}>
          {intl.formatMessage(m.inBasis)}
          <select id={ids.basis} className={input} value={indicator.basis} disabled={!editable} onChange={(e) => onChange({ basis: e.target.value })}>
            {(BASES as readonly string[]).includes(indicator.basis) ? null : <option value={indicator.basis}>{indicator.basis}</option>}
            {BASES.map((b) => (
              <option key={b} value={b}>
                {b}
              </option>
            ))}
          </select>
        </label>
        <div className={label}>
          <label htmlFor={ids.periodDate}>{intl.formatMessage(m.inPeriodDate)}</label>
          <input id={ids.periodDate} aria-describedby={`${ids.periodDate}-hint`} className={cx(input, css({ fontFamily: 'mono' }))} list={ids.dates} required value={indicator.periodDate} readOnly={!editable} onChange={(e) => onChange({ periodDate: e.target.value })} />
          <datalist id={ids.dates}>
            {dates.map((d) => (
              <option key={d.path} value={d.path}>
                {intl.formatMessage(m.inDateName, { date: d.label })}
              </option>
            ))}
          </datalist>
          <span id={`${ids.periodDate}-hint`} className={muted}>
            {intl.formatMessage(m.inPeriodDateHint)}
          </span>
        </div>
      </div>
      <label className={label} htmlFor={ids.description}>
        {intl.formatMessage(m.inDescription)}
        <textarea id={ids.description} className={input} rows={2} value={indicator.description} readOnly={!editable} onChange={(e) => onChange({ description: e.target.value })} />
      </label>
    </div>
  )
}

function Calculates({ indicator, editable, onChange }: { indicator: Indicator; editable: boolean; onChange: (next: Indicator) => void }) {
  const intl = useIntl()
  const ids = { heading: useId(), calculation: useId(), per: useId(), pick: useId(), value: useId(), from: useId(), to: useId(), unit: useId() }
  const set = (next: Partial<Indicator>) => onChange({ ...indicator, ...next })
  const value = indicator.value
  return (
    <div className={panel} role="group" aria-labelledby={ids.heading}>
      <h3 id={ids.heading} className={subheading}>
        {intl.formatMessage(m.inCalculates)}
      </h3>
      <div className={grid}>
        <label className={label} htmlFor={ids.calculation}>
          {intl.formatMessage(m.inCalculation)}
          <select id={ids.calculation} className={input} value={indicator.calculation} disabled={!editable} onChange={(e) => onChange(withCalculation(indicator, e.target.value as Calculation))}>
            {CALCULATIONS.map((c) => (
              <option key={c} value={c}>
                {intl.formatMessage(m.inCalculationName, { calculation: icu(c) })}
              </option>
            ))}
          </select>
        </label>
        {indicator.calculation === 'rate' ? (
          <label className={label} htmlFor={ids.per}>
            {intl.formatMessage(m.inRatePer)}
            <select id={ids.per} className={input} value={indicator.ratePer} disabled={!editable} onChange={(e) => set({ ratePer: Number(e.target.value) })}>
              {RATE_FACTORS.map((f) => (
                <option key={f} value={f}>
                  {intl.formatNumber(f)}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        {takesValue(indicator.calculation) && (indicator.counts === 'people' || indicator.counts === 'enrollments') ? (
          <label className={label} htmlFor={ids.pick}>
            {intl.formatMessage(m.inPick)}
            <select id={ids.pick} className={input} value={indicator.pick} disabled={!editable} onChange={(e) => set({ pick: e.target.value === 'last' ? 'last' : 'first' })}>
              <option value="first">{intl.formatMessage(m.inPickName, { pick: 'first' })}</option>
              <option value="last">{intl.formatMessage(m.inPickName, { pick: 'last' })}</option>
            </select>
          </label>
        ) : null}
      </div>
      <label className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
        <input type="checkbox" checked={indicator.cumulative} disabled={!editable} onChange={(e) => set({ cumulative: e.target.checked })} />
        {intl.formatMessage(m.inCumulative)}
      </label>
      {takesValue(indicator.calculation) ? (
        <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '2', border: 'none', p: '0', m: '0' })}>
          <legend className={css({ fontWeight: 'medium', mb: '1' })}>{intl.formatMessage(m.inValue)}</legend>
          <div className={row}>
            <label className={row}>
              <input type="radio" name={ids.value} checked={value.kind === 'expression'} disabled={!editable} onChange={() => set({ value: { kind: 'expression', expression: '' } })} />
              {intl.formatMessage(m.inValueField)}
            </label>
            <label className={row}>
              <input type="radio" name={ids.value} checked={value.kind === 'between'} disabled={!editable} onChange={() => set({ value: { kind: 'between', from: '', to: indicator.periodDate, unit: 'days' } })} />
              {intl.formatMessage(m.inValueBetween)}
            </label>
          </div>
          {value.kind === 'expression' ? (
            <input
              aria-label={intl.formatMessage(m.inValueExpression)}
              className={cx(input, css({ fontFamily: 'mono' }))}
              value={value.expression}
              readOnly={!editable}
              onChange={(e) => set({ value: { kind: 'expression', expression: e.target.value } })}
            />
          ) : (
            <div className={grid}>
              <label className={label} htmlFor={ids.from}>
                {intl.formatMessage(m.inValueFrom)}
                <input id={ids.from} className={cx(input, css({ fontFamily: 'mono' }))} value={value.from} readOnly={!editable} onChange={(e) => set({ value: { ...value, from: e.target.value } })} />
              </label>
              <label className={label} htmlFor={ids.to}>
                {intl.formatMessage(m.inValueTo)}
                <input id={ids.to} className={cx(input, css({ fontFamily: 'mono' }))} value={value.to} readOnly={!editable} onChange={(e) => set({ value: { ...value, to: e.target.value } })} />
              </label>
              <label className={label} htmlFor={ids.unit}>
                {intl.formatMessage(m.inValueUnit)}
                <select id={ids.unit} className={input} value={value.unit} disabled={!editable} onChange={(e) => set({ value: { ...value, unit: e.target.value as BetweenUnit } })}>
                  {BETWEEN_UNITS.map((u) => (
                    <option key={u} value={u}>
                      {intl.formatMessage(m.inUnitName, { unit: u })}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )}
        </fieldset>
      ) : null}
    </div>
  )
}

/** Where a percentage's, rate's or ratio's denominator comes from (#864): its records, or a Population Estimate type the Population packages declare; and *Remove denominator*. */
function DenominatorFrom({ indicator, editable, types, onChange }: { indicator: Indicator; editable: boolean; types: readonly EstimateType[]; onChange: (next: (i: Indicator) => Indicator) => void }) {
  const intl = useIntl()
  const current = indicator.denominatorEstimate
  const offered = current === null || types.some((t) => t.code === current) ? types : [...types, { code: current, title: current }]
  return (
    <div className={row}>
      <label className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
        {intl.formatMessage(m.inDenominatorFrom)}
        <select className={control} value={current ?? ''} disabled={!editable} onChange={(e) => onChange((i) => withDenominatorEstimate(i, e.target.value === '' ? null : e.target.value))}>
          <option value="">{intl.formatMessage(m.inFromRecords)}</option>
          {offered.map((t) => (
            <option key={t.code} value={t.code}>
              {intl.formatMessage(m.inFromEstimate, { title: t.title })}
            </option>
          ))}
        </select>
      </label>
      {offered.length === 0 ? <span className={muted}>{intl.formatMessage(m.inNoEstimateTypes)}</span> : null}
      {editable ? (
        <button type="button" className={small} onClick={() => onChange(removeDenominator)}>
          {intl.formatMessage(m.inRemoveDenominator)}
        </button>
      ) : null}
    </div>
  )
}

/** A denominator that is a Population Estimate type (#553, #864): each place's estimate for the period, not counted from records. */
function EstimateDenominator({ indicator, editable, types, onChange }: { indicator: Indicator; editable: boolean; types: readonly EstimateType[]; onChange: (next: (i: Indicator) => Indicator) => void }) {
  const intl = useIntl()
  const heading = useId()
  return (
    <div className={panel} role="group" aria-labelledby={heading}>
      <h3 id={heading} className={subheading}>
        {intl.formatMessage(m.inDenominator)}
      </h3>
      <DenominatorFrom indicator={indicator} editable={editable} types={types} onChange={onChange} />
      <p className={muted}>{intl.formatMessage(m.inEstimateHint)}</p>
    </div>
  )
}

function SideEditor({ title, hint, tree, context, onChange, children }: { title: string; hint: string; tree: Criteria; context: TreeContext; onChange: (tree: Criteria) => void; children?: ReactNode }) {
  const intl = useIntl()
  const heading = useId()
  return (
    <div className={panel} role="group" aria-labelledby={heading}>
      <div className={css({ display: 'flex', justifyContent: 'space-between', gap: '2', flexWrap: 'wrap', alignItems: 'center' })}>
        <h3 id={heading} className={subheading}>
          {title}
        </h3>
        {children}
      </div>
      <p className={muted}>{hint}</p>
      <NodeEditor
        node={tree}
        path={[]}
        side={title}
        context={context}
        onEdit={(path, f) => {
          const next = update(tree, path, f)
          if (next !== null && (next as { kind: string }).kind === 'group') onChange(next as Criteria)
        }}
      />
      <div>
        <p className={muted}>{intl.formatMessage(m.inCompiled)}</p>
        <textarea className={cx(mono, css({ width: '100%', resize: 'vertical', border: 'none' }))} rows={3} readOnly aria-label={intl.formatMessage(m.inCompiledOf, { side: title })} value={compileCriteria(tree)} />
      </div>
    </div>
  )
}

/** `root` with the node at `path` replaced by what `f` returns; null removes it. */
function update(root: CriteriaNode, path: number[], f: (node: CriteriaNode) => CriteriaNode | null): CriteriaNode | null {
  if (path.length === 0) return f(root)
  if ((root as { kind: string }).kind !== 'group') return root
  const group = root as Criteria
  const [head, ...rest] = path
  return { ...group, children: group.children.flatMap((child, n) => (n === head ? [update(child, rest, f)].filter((c): c is CriteriaNode => c !== null) : [child])) }
}

interface TreeContext {
  editable: boolean
  dates: ReturnType<typeof declaredDates>
  others: readonly Other[]
}

const pathText = (path: number[]) => ['1', ...path.map((n) => String(n + 1))].join('.')

interface NodeProps {
  node: CriteriaNode
  path: number[]
  side: string
  context: TreeContext
  onEdit: (path: number[], f: (node: CriteriaNode) => CriteriaNode | null) => void
}

function NodeEditor({ node, path, side, context, onEdit }: NodeProps) {
  const intl = useIntl()
  const at = `${side} ${pathText(path)}`
  const { editable } = context
  const remove = editable ? (
    <button type="button" className={small} onClick={() => onEdit(path, () => null)}>
      {intl.formatMessage(m.inRemove, { at })}
    </button>
  ) : null
  const listId = useId()
  const kind = (node as { kind: string }).kind
  if (kind === 'fhirpath') {
    const raw = node as { kind: 'fhirpath'; expression: string }
    return (
      <div className={row}>
        <textarea
          className={cx(input, css({ flex: '1', fontFamily: 'mono', fontSize: 'xs' }))}
          rows={2}
          aria-label={intl.formatMessage(m.inRaw, { at })}
          value={raw.expression}
          readOnly={!editable}
          onChange={(e) => onEdit(path, () => ({ kind: 'fhirpath', expression: e.target.value }))}
        />
        {path.length > 0 ? remove : null}
      </div>
    )
  }
  if (kind === 'condition') {
    const condition = node as { kind: 'condition'; input: string; op: Operator; value: boolean | number | string }
    const valueKind = kindOf(condition.value)
    const set = (patch: Partial<typeof condition>) => onEdit(path, () => ({ ...condition, ...patch }))
    return (
      <div className={row}>
        <input className={cx(input, css({ fontFamily: 'mono' }))} list={listId} aria-label={intl.formatMessage(m.inInput, { at })} value={condition.input} readOnly={!editable} onChange={(e) => set({ input: e.target.value })} />
        <datalist id={listId}>
          {CONDITION_INPUTS.map((i) => (
            <option key={i} value={i} />
          ))}
        </datalist>
        <select className={input} aria-label={intl.formatMessage(m.inOperator, { at })} value={condition.op} disabled={!editable} onChange={(e) => set({ op: e.target.value as Operator })}>
          {operatorsFor(valueKind).map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
        <select
          className={input}
          aria-label={intl.formatMessage(m.inKind, { at })}
          value={valueKind}
          disabled={!editable}
          onChange={(e) => {
            const next = e.target.value as LiteralKind
            set({ value: blank(next), op: operatorsFor(next).includes(condition.op) ? condition.op : '=' })
          }}
        >
          {(['boolean', 'number', 'code'] as const).map((k) => (
            <option key={k} value={k}>
              {intl.formatMessage(m.inKindName, { kind: k })}
            </option>
          ))}
        </select>
        {valueKind === 'boolean' ? (
          <select className={input} aria-label={intl.formatMessage(m.inLiteral, { at })} value={String(condition.value)} disabled={!editable} onChange={(e) => set({ value: e.target.value === 'true' })}>
            <option value="true">true</option>
            <option value="false">false</option>
          </select>
        ) : (
          <input
            className={cx(input, css({ width: valueKind === 'number' ? '6rem' : '10rem' }))}
            aria-label={intl.formatMessage(m.inLiteral, { at })}
            type={valueKind === 'number' ? 'number' : 'text'}
            value={shown(condition.value)}
            readOnly={!editable}
            onChange={(e) => set({ value: literal(valueKind, e.target.value) })}
          />
        )}
        {remove}
      </div>
    )
  }
  if (kind === 'member') {
    const member = node as { kind: 'member'; measure: string; side: Side }
    const known = context.others.some((o) => o.canonical === member.measure)
    return (
      <div className={row}>
        <span>{intl.formatMessage(m.inMemberIsIn)}</span>
        <select className={input} aria-label={intl.formatMessage(m.inMemberSide, { at })} value={member.side} disabled={!editable} onChange={(e) => onEdit(path, () => ({ ...member, side: e.target.value as Side }))}>
          {SIDES.map((s) => (
            <option key={s} value={s}>
              {intl.formatMessage(m.inSideName, { side: s })}
            </option>
          ))}
        </select>
        <span>{intl.formatMessage(m.inMemberOf)}</span>
        <select className={input} aria-label={intl.formatMessage(m.inMemberMeasure, { at })} value={member.measure} disabled={!editable} onChange={(e) => onEdit(path, () => ({ ...member, measure: e.target.value }))}>
          {known ? null : <option value={member.measure}>{member.measure === '' ? intl.formatMessage(m.inMemberChoose) : member.measure}</option>}
          {context.others.map((o) => (
            <option key={o.canonical} value={o.canonical}>
              {intl.formatMessage(m.inMemberOption, { title: o.title, canonical: o.canonical })}
            </option>
          ))}
        </select>
        {remove}
      </div>
    )
  }
  if (kind === 'date') return <DateEditor condition={node as DateCondition} path={path} at={at} context={context} onEdit={onEdit} remove={remove} />
  const group = node as Group<Atom>
  const add = (child: CriteriaNode) => onEdit(path, (n) => ((n as { kind: string }).kind === 'group' ? { ...(n as Criteria), children: [...(n as Criteria).children, child] } : n))
  return (
    <div
      role="group"
      aria-label={intl.formatMessage(m.inGroup, { at, join: group.join })}
      className={css({
        display: 'flex',
        flexDirection: 'column',
        gap: '2',
        borderLeft: '3px solid',
        borderColor: group.join === 'not' ? 'danger.text' : group.join === 'any' ? 'accent.default' : 'border',
        pl: '3',
        py: '1',
      })}
    >
      <div className={row}>
        <select
          className={cx(input, css({ fontWeight: 'semibold' }))}
          aria-label={intl.formatMessage(m.inJoin, { at })}
          value={group.join}
          disabled={!editable}
          onChange={(e) => onEdit(path, (n) => ({ ...(n as Criteria), join: e.target.value as Join }))}
        >
          {JOINS.map((j) => (
            <option key={j} value={j}>
              {intl.formatMessage(m.inJoinName, { join: j })}
            </option>
          ))}
        </select>
        {editable ? (
          <>
            <button type="button" className={small} onClick={() => add({ kind: 'condition', input: CONDITION_INPUTS[0], op: '=', value: '' })}>
              {intl.formatMessage(m.inAddCondition, { at })}
            </button>
            <button type="button" className={small} onClick={() => add({ kind: 'date', input: context.dates[0]?.path ?? '', op: 'inPeriod' })}>
              {intl.formatMessage(m.inAddDate, { at })}
            </button>
            <button type="button" className={small} onClick={() => add({ kind: 'member', measure: context.others[0]?.canonical ?? '', side: 'numerator' })}>
              {intl.formatMessage(m.inAddMember, { at })}
            </button>
            <button type="button" className={small} onClick={() => add({ kind: 'group', join: 'all', children: [] })}>
              {intl.formatMessage(m.inAddGroup, { at })}
            </button>
            <button type="button" className={small} onClick={() => add({ kind: 'fhirpath', expression: '' })}>
              {intl.formatMessage(m.inAddRaw, { at })}
            </button>
            {path.length > 0 ? remove : null}
          </>
        ) : null}
      </div>
      {group.children.length === 0 ? <p className={muted}>{intl.formatMessage(m.inEmptyGroup)}</p> : null}
      {group.children.map((child, n) => (
        <NodeEditor key={n} node={child} path={[...path, n]} side={side} context={context} onEdit={onEdit} />
      ))}
    </div>
  )
}

const DATE_OPS_SHOWN: readonly DateOp[] = ['inPeriod', 'on', 'before', 'after', 'between', 'within', 'previous', 'empty', 'notEmpty']

/** A date condition with `op`, keeping its input and whatever of its anchors still fits. */
function withOp(condition: DateCondition, op: DateOp): DateCondition {
  const on = condition.input
  const anchor: Anchor = 'anchor' in condition ? condition.anchor : 'from' in condition ? condition.from : { at: 'periodEnd' }
  switch (op) {
    case 'inPeriod':
    case 'empty':
    case 'notEmpty':
      return { kind: 'date', input: on, op }
    case 'on':
    case 'before':
    case 'after':
      return { kind: 'date', input: on, op, anchor }
    case 'between':
      return { kind: 'date', input: on, op, from: { at: 'periodStart' }, to: { at: 'periodEnd' } }
    case 'within':
      return { kind: 'date', input: on, op, amount: 4, unit: 'weeks', direction: 'after', anchor }
    case 'previous':
      return { kind: 'date', input: on, op, periods: 1 }
  }
}

function AnchorField({ anchor, at, name, editable, onChange }: { anchor: Anchor; at: string; name: string; editable: boolean; onChange: (anchor: Anchor) => void }) {
  const intl = useIntl()
  return (
    <>
      <select
        className={input}
        aria-label={intl.formatMessage(m.inAnchor, { at, name })}
        value={anchor.at}
        disabled={!editable}
        onChange={(e) => onChange(e.target.value === 'date' ? { at: 'date', date: '2026-01-01' } : { at: e.target.value === 'periodStart' ? 'periodStart' : 'periodEnd' })}
      >
        {(['periodStart', 'periodEnd', 'date'] as const).map((a) => (
          <option key={a} value={a}>
            {intl.formatMessage(m.inAnchorName, { anchor: a })}
          </option>
        ))}
      </select>
      {anchor.at === 'date' ? (
        <input className={input} type="date" aria-label={intl.formatMessage(m.inAnchorDate, { at, name })} value={anchor.date} readOnly={!editable} onChange={(e) => onChange({ at: 'date', date: e.target.value })} />
      ) : null}
    </>
  )
}

function DateEditor({ condition, path, at, context, onEdit, remove }: { condition: DateCondition; path: number[]; at: string; context: TreeContext; onEdit: NodeProps['onEdit']; remove: ReactNode }) {
  const intl = useIntl()
  const { editable } = context
  const set = (next: DateCondition) => onEdit(path, () => next)
  const listId = useId()
  return (
    <div className={row}>
      <input
        className={cx(input, css({ fontFamily: 'mono', maxWidth: '22rem' }))}
        list={listId}
        aria-label={intl.formatMessage(m.inDateInput, { at })}
        value={condition.input}
        readOnly={!editable}
        onChange={(e) => set({ ...condition, input: e.target.value })}
      />
      <datalist id={listId}>
        {context.dates.map((d) => (
          <option key={d.path} value={d.path}>
            {intl.formatMessage(m.inDateName, { date: d.label })}
          </option>
        ))}
      </datalist>
      <select className={input} aria-label={intl.formatMessage(m.inDateOp, { at })} value={condition.op} disabled={!editable} onChange={(e) => set(withOp(condition, e.target.value as DateOp))}>
        {DATE_OPS_SHOWN.map((op) => (
          <option key={op} value={op}>
            {intl.formatMessage(m.inDateOpName, { op })}
          </option>
        ))}
      </select>
      {condition.op === 'on' || condition.op === 'before' || condition.op === 'after' ? <AnchorField anchor={condition.anchor} at={at} name="" editable={editable} onChange={(anchor) => set({ ...condition, anchor })} /> : null}
      {condition.op === 'between' ? (
        <>
          <AnchorField anchor={condition.from} at={at} name={intl.formatMessage(m.inFrom)} editable={editable} onChange={(from) => set({ ...condition, from })} />
          <span>{intl.formatMessage(m.inAnd)}</span>
          <AnchorField anchor={condition.to} at={at} name={intl.formatMessage(m.inTo)} editable={editable} onChange={(to) => set({ ...condition, to })} />
        </>
      ) : null}
      {condition.op === 'within' ? (
        <>
          <input
            className={cx(input, css({ width: '5rem' }))}
            type="number"
            min={1}
            aria-label={intl.formatMessage(m.inAmount, { at })}
            value={condition.amount}
            readOnly={!editable}
            onChange={(e) => set({ ...condition, amount: Math.max(1, Number.parseInt(e.target.value, 10) || 1) })}
          />
          <select className={input} aria-label={intl.formatMessage(m.inUnit, { at })} value={condition.unit} disabled={!editable} onChange={(e) => set({ ...condition, unit: e.target.value as DateUnit })}>
            {DATE_UNITS.map((u) => (
              <option key={u} value={u}>
                {intl.formatMessage(m.inUnitName, { unit: u })}
              </option>
            ))}
          </select>
          <select className={input} aria-label={intl.formatMessage(m.inDirection, { at })} value={condition.direction} disabled={!editable} onChange={(e) => set({ ...condition, direction: e.target.value === 'before' ? 'before' : 'after' })}>
            <option value="before">{intl.formatMessage(m.inDirectionName, { direction: 'before' })}</option>
            <option value="after">{intl.formatMessage(m.inDirectionName, { direction: 'after' })}</option>
          </select>
          <AnchorField anchor={condition.anchor} at={at} name="" editable={editable} onChange={(anchor) => set({ ...condition, anchor })} />
        </>
      ) : null}
      {condition.op === 'previous' ? (
        <>
          <input
            className={cx(input, css({ width: '5rem' }))}
            type="number"
            min={1}
            max={60}
            aria-label={intl.formatMessage(m.inPeriods, { at })}
            value={condition.periods}
            readOnly={!editable}
            onChange={(e) => set({ ...condition, periods: Math.min(60, Math.max(1, Number.parseInt(e.target.value, 10) || 1)) })}
          />
          <span>{intl.formatMessage(m.inPeriodsWord)}</span>
        </>
      ) : null}
      {remove}
    </div>
  )
}

function Disaggregations({ indicator, editable, sets, onChange }: { indicator: Indicator; editable: boolean; sets: { canonical: string; title: string }[]; onChange: (next: Partial<Indicator>) => void }) {
  const intl = useIntl()
  const heading = useId()
  const list = indicator.disaggregations
  const set = (at: number, next: Indicator['disaggregations'][number] | null) => onChange({ disaggregations: list.flatMap((d, n) => (n === at ? (next === null ? [] : [next]) : [d])) })
  return (
    <div className={panel} role="group" aria-labelledby={heading}>
      <h3 id={heading} className={subheading}>
        {intl.formatMessage(m.inDisaggregations)}
      </h3>
      <p className={muted}>{intl.formatMessage(m.inDisaggregationsHint)}</p>
      {list.length === 0 ? <p className={muted}>{intl.formatMessage(m.inNoDisaggregations)}</p> : null}
      {list.map((d, n) => {
        const at = n + 1
        const remove = editable ? (
          <button type="button" className={small} onClick={() => set(n, null)}>
            {intl.formatMessage(m.inRemoveDisaggregation, { at })}
          </button>
        ) : null
        if (d.kind === 'age')
          return (
            <div key={n} className={row}>
              <span>{intl.formatMessage(m.inByAge)}</span>
              <select className={input} aria-label={intl.formatMessage(m.inAgeSet, { at })} value={d.set} disabled={!editable} onChange={(e) => set(n, { kind: 'age', set: e.target.value })}>
                {sets.some((s) => s.canonical === d.set) ? null : <option value={d.set}>{d.set}</option>}
                {sets.map((s) => (
                  <option key={s.canonical} value={s.canonical}>
                    {intl.formatMessage(m.inMemberOption, { title: s.title, canonical: s.canonical })}
                  </option>
                ))}
              </select>
              {remove}
            </div>
          )
        return (
          <div key={n} className={row}>
            <span>{intl.formatMessage(m.inByCode)}</span>
            <input className={cx(input, css({ width: '10rem' }))} aria-label={intl.formatMessage(m.inCodedName, { at })} value={d.name} readOnly={!editable} onChange={(e) => set(n, { ...d, name: e.target.value })} />
            <input
              className={cx(input, css({ flex: '1', fontFamily: 'mono' }))}
              aria-label={intl.formatMessage(m.inCodedCriteria, { at })}
              value={d.criteria}
              readOnly={!editable}
              onChange={(e) => set(n, { ...d, criteria: e.target.value })}
            />
            {remove}
          </div>
        )
      })}
      {editable ? (
        <div className={row}>
          <button type="button" className={small} disabled={sets.length === 0} onClick={() => onChange({ disaggregations: [...list, { kind: 'age', set: sets[0]?.canonical ?? '' }] })}>
            {intl.formatMessage(m.inAddAge)}
          </button>
          <button type="button" className={small} onClick={() => onChange({ disaggregations: [...list, { kind: 'coded', name: 'sex', criteria: '%patient.gender' }] })}>
            {intl.formatMessage(m.inAddCoded)}
          </button>
        </div>
      ) : null}
    </div>
  )
}

// ---------------------------------------------------------------------------------------------
// Try

const today = () => new Date().toISOString().slice(0, 10)

/** What *Save as test* needs from the editor: whether it may (the definition saved), and how. */
interface TestSaving {
  allowed: boolean
  why: string
  pending: boolean
  saved: string | null
  failed: string | null
  onSave: (tried: { periodType: PeriodType; period: string; result: StudioIndicatorTry }) => void
}

function TryPanel({ id, measure, explained, calculation, test }: { id: string; measure: Record<string, unknown>; explained: Explained[]; calculation: Calculation; test: TestSaving }) {
  const intl = useIntl()
  const platform = usePlatform()
  const ids = { heading: useId(), type: useId(), period: useId() }
  const [periodType, setPeriodType] = useState<PeriodType>('monthly')
  const [period, setPeriod] = useState(() => periodOf('monthly', today()))
  const settled = useSettled({ measure, periodType, period, conditions: explained.map((e) => e.expression) })
  const result = useQuery({
    queryKey: ['studio', 'indicators', 'try', id, settled] as const,
    queryFn: ({ signal }) => tryIndicator(platform.transport, id, settled, signal),
    retry: false,
    enabled: settled.period.trim() !== '',
  })
  const data = result.data
  return (
    <aside className={cx(panel, sticky)} aria-labelledby={ids.heading}>
      <h2 id={ids.heading} className={subheading}>
        {intl.formatMessage(m.inTry)}
      </h2>
      <p className={muted}>{intl.formatMessage(m.inTryHint)}</p>
      <div className={grid}>
        <label className={label} htmlFor={ids.type}>
          {intl.formatMessage(m.inPeriodType)}
          <select
            id={ids.type}
            className={input}
            value={periodType}
            onChange={(e) => {
              const next = e.target.value as PeriodType
              setPeriodType(next)
              setPeriod(periodOf(next, today()))
            }}
          >
            {PERIOD_TYPES.map((t) => (
              <option key={t} value={t}>
                {intl.formatMessage(m.inPeriodTypeName, { type: t })}
              </option>
            ))}
          </select>
        </label>
        <div className={label}>
          <label htmlFor={ids.period}>{intl.formatMessage(m.inPeriod)}</label>
          <input id={ids.period} aria-describedby={`${ids.period}-hint`} className={input} value={period} onChange={(e) => setPeriod(e.target.value.trim())} />
          <span id={`${ids.period}-hint`} className={muted}>
            {intl.formatMessage(m.inPeriodExample, { type: periodType })}
          </span>
        </div>
      </div>
      {result.isFetching ? <p role="status">{intl.formatMessage(m.inTrying)}</p> : null}
      {result.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.inTryFailed, { reason: failure(intl, result.error) })}
        </p>
      ) : null}
      {data === undefined ? null : (
        <>
          <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', alignItems: 'flex-start' })}>
            <button
              type="button"
              className={small}
              disabled={!test.allowed || test.pending || result.isFetching}
              aria-describedby={test.why === '' ? undefined : `${ids.heading}-test`}
              onClick={() => test.onSave({ periodType: data.period.periodType, period: data.period.id, result: data })}
            >
              {intl.formatMessage(m.inSaveTest)}
            </button>
            {test.why === '' ? null : (
              <span id={`${ids.heading}-test`} className={muted}>
                {test.why}
              </span>
            )}
            {test.saved === null ? null : <span role="status">{intl.formatMessage(m.inTestSaved, { path: test.saved })}</span>}
            {test.failed === null ? null : (
              <span role="alert" className={alert}>
                {intl.formatMessage(m.inFailed, { reason: test.failed })}
              </span>
            )}
          </div>
          <TryResult data={data} calculation={calculation} explained={explained} />
        </>
      )}
    </aside>
  )
}

/** A figure as the server sent it (a decimal arrives exact), or a dash. */
const number = (value: { toString: () => string } | null | undefined) => (value === null || value === undefined ? '—' : value.toString())

function TryResult({ data, calculation, explained }: { data: StudioIndicatorTry; calculation: Calculation; explained: Explained[] }) {
  const intl = useIntl()
  const ratio = data.denominator !== null && data.denominator !== undefined
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <p className={muted}>{intl.formatMessage(m.inPeriodRange, { id: data.period.id, start: data.period.start, end: data.period.end })}</p>
      <div role="group" aria-label={intl.formatMessage(m.inFigure)}>
        <p className={figure}>
          {data.value === null || data.value === undefined ? '—' : intl.formatMessage(m.inValueShown, { value: number(data.value), calculation: icu(data.calculation), per: data.ratePer ?? 0 })}
        </p>
        <p>{ratio ? intl.formatMessage(m.inParts, { count: data.count, denominator: data.denominator ?? 0 }) : takesValueText(intl, data, calculation)}</p>
      </div>
      {data.disaggregations.map((d) => (
        <table key={d.name} className={table}>
          <caption className={css({ textAlign: 'left', fontWeight: 'semibold', pb: '1' })}>{intl.formatMessage(m.inByTitle, { title: d.title })}</caption>
          <thead>
            <tr>
              <th scope="col">{intl.formatMessage(m.inBandColumn)}</th>
              <th scope="col">{intl.formatMessage(ratio ? m.inNumeratorColumn : m.inCountColumn)}</th>
              {ratio ? <th scope="col">{intl.formatMessage(m.inDenominatorColumn)}</th> : null}
              <th scope="col">{intl.formatMessage(m.inValueColumn)}</th>
            </tr>
          </thead>
          <tbody>
            {d.bands.map((b) => (
              <tr key={b.code}>
                <th scope="row">{b.label}</th>
                <td>{b.count}</td>
                {ratio ? <td>{number(b.denominator)}</td> : null}
                <td>{number(b.value)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ))}
      {data.errors.length === 0 ? null : (
        <ul className={css({ pl: '5', listStyle: 'disc' })} aria-label={intl.formatMessage(m.inErrors)}>
          {data.errors.map((e) => (
            <li key={e} className={alert}>
              {e}
            </li>
          ))}
        </ul>
      )}
      <Patients patients={data.patients} ratio={ratio} names={data.disaggregations.map((d) => d.title)} explained={explained} />
    </div>
  )
}

function takesValueText(intl: IntlShape, data: StudioIndicatorTry, calculation: Calculation): string {
  if (data.values === null || data.values === undefined || !takesValue(calculation)) return intl.formatMessage(m.inCountShown, { count: data.count })
  return intl.formatMessage(m.inValuesShown, { count: data.values.count, sum: number(data.values.sum), minimum: number(data.values.minimum), maximum: number(data.values.maximum) })
}

function Patients({ patients, ratio, names, explained }: { patients: StudioIndicatorPatient[]; ratio: boolean; names: string[]; explained: Explained[] }) {
  const intl = useIntl()
  const heading = useId()
  const counted = patients.filter((p) => p.counted).length
  return (
    <div role="region" aria-labelledby={heading} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <h3 id={heading} className={css({ fontWeight: 'semibold' })}>
        {intl.formatMessage(m.inPatients, { counted, total: patients.length })}
      </h3>
      <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', listStyle: 'none', p: '0', m: '0' })}>
        {patients.map((p) => (
          <li key={p.path}>
            <details>
              <summary className={css({ cursor: 'pointer' })}>
                <span className={p.counted ? good : muted} aria-hidden>
                  {p.counted ? '✓ ' : '✗ '}
                </span>
                {intl.formatMessage(m.inPatientLine, {
                  name: p.display ?? p.name,
                  counted: String(p.counted),
                  count: p.count,
                  denominator: ratio ? number(p.denominator) : 'none',
                })}
              </summary>
              <p className={muted}>{p.name}</p>
              <ul className={css({ pl: '4', fontSize: 'sm' })}>
                {p.records.map((r) => (
                  <li key={r.reference}>
                    <code>{r.reference}</code> {r.date ?? ''} — {intl.formatMessage(m.inReason, { reason: icu(r.reason) })}
                    {r.error === null || r.error === undefined ? '' : `: ${r.error}`}
                    {r.value === null || r.value === undefined ? '' : ` · ${intl.formatMessage(m.inRecordValue, { value: r.value, picked: String(r.picked) })}`}
                    {r.bands.length === 0 ? '' : ` · ${r.bands.map((b, n) => `${names[n] ?? ''}: ${b}`).join(', ')}`}
                    {/* The marks answer the conditions asked; a stale answer (asked before an edit) is not shown. */}
                    {r.conditions.length === explained.length && explained.length > 0 ? <ConditionMarks explained={explained} held={r.conditions} ratio={ratio} /> : null}
                  </li>
                ))}
              </ul>
            </details>
          </li>
        ))}
      </ul>
    </div>
  )
}

const MARKS: Record<StudioHeld, string> = { held: '✓', 'not-held': '✗', error: '!' }

/** Whether each condition of the numerator and denominator held on one record (#864), each side's
 * conditions as its tree nests them. */
function ConditionMarks({ explained, held, ratio }: { explained: Explained[]; held: StudioHeld[]; ratio: boolean }) {
  const intl = useIntl()
  const sides = (['numerator', 'denominator'] as const).filter((side) => explained.some((e) => e.side === side))
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', pl: '2', py: '1' })}>
      {sides.map((side) => (
        <div key={side}>
          <span className={muted}>{intl.formatMessage(m.inExplainSide, { side: ratio ? side : 'counted' })}</span>
          <ul className={css({ listStyle: 'none', p: '0', m: '0' })}>
            {explained.map((e, n) =>
              e.side !== side ? null : (
                <li key={n} className={css({ display: 'flex', gap: '1', alignItems: 'baseline' })} style={{ paddingLeft: `${(e.depth - 1) * 1}rem` }}>
                  <span className={held[n] === 'held' ? good : held[n] === 'error' ? alert : muted} aria-hidden>
                    {MARKS[held[n] ?? 'error']}
                  </span>
                  <span className={css({ srOnly: true })}>{intl.formatMessage(m.inHeld, { held: icu(held[n] ?? 'error') })}</span>
                  <ConditionLabel node={e} />
                </li>
              ),
            )}
          </ul>
        </div>
      ))}
    </div>
  )
}

function ConditionLabel({ node }: { node: Explained }) {
  const intl = useIntl()
  const kind = (node.node as { kind: string }).kind
  if (kind === 'group') return <span>{intl.formatMessage(m.inJoinName, { join: (node.node as Criteria).join })}</span>
  return <code className={css({ fontSize: 'xs', overflowWrap: 'anywhere' })}>{node.expression}</code>
}

/** Where Reports lay the indicator out (studio.md § Indicators, *HMIS mapping*; #864), read-only:
 * mappings are edited in the Reporting package's Report designer. */
function HmisMapping({ url }: { url: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const heading = useId()
  const mappings = useQuery({
    queryKey: ['studio', 'indicators', 'mappings', url] as const,
    queryFn: ({ signal }) => indicatorMappings(platform.transport, url, signal),
    enabled: url !== '',
    retry: false,
  })
  const found = mappings.data ?? []
  return (
    <div role="region" aria-labelledby={heading} className={panel}>
      <h3 id={heading} className={css({ fontWeight: 'semibold' })}>
        {intl.formatMessage(m.inMapping)}
      </h3>
      <p className={muted}>{intl.formatMessage(m.inMappingHint)}</p>
      {mappings.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.inFailed, { reason: failure(intl, mappings.error) })}
        </p>
      ) : mappings.isSuccess && found.length === 0 ? (
        <p>{intl.formatMessage(m.inMappingNone)}</p>
      ) : (
        <ul className={css({ pl: '5', listStyle: 'disc', display: 'flex', flexDirection: 'column', gap: '1' })}>
          {found.map((f) => (
            <li key={`${f.packageId} ${f.report} ${f.section} ${f.label}`}>
              <Link {...editorOf({ kind: 'report', path: `configuration/reports/${f.report}.json`, table: null }, f.packageId)}>{intl.formatMessage(m.inMappingRow, { title: f.title, section: f.section, label: f.label })}</Link>{' '}
              <span className={muted}>
                {intl.formatMessage(m.inMappingWhere, { package: f.packageId, published: String(f.published), pinned: f.indicator.split('|')[1] ?? '', sent: String(f.sentToDhis2) })}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
