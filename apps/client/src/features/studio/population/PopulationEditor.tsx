import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { populationMessages as m, studioMessages as sm } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button, control, field } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { refusalText } from '../wording'
import { checkCsv, givenTypes, grouped, readEstimates, template, type Checked, type Estimates } from './estimates'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '5xl' })
const muted = css({ color: 'fg.muted' })
const alert = css({ color: 'danger.text' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const grid = css({ width: '100%', borderCollapse: 'collapse', '& th, & td': { textAlign: 'left', p: '2', borderBottom: '1px solid', borderColor: 'border' } })
const numeric = css({ textAlign: 'right !important', fontVariantNumeric: 'tabular-nums' })
const actions = css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center' })
const area = css({ minHeight: '8rem', fontFamily: 'mono', fontSize: 'sm' })

/**
 * A year's Population Estimates (analytics.md § Age Band Sets and Population Estimates; ADR 0071;
 * #553): the estimate types, given per place or derived as a percentage of another, and the
 * figures the draft holds. The HMIS lead downloads the CSV template, fills it (typically from
 * UBOS projections) and imports it: central checks every place code against the facility
 * registry and the NHFR, refusing the file with each bad row named, and shows each district's
 * totals, derived types included, before *Save* writes the figures to the draft through the file
 * editor's `If-Match` path.
 */
export function PopulationEditor({ id, path }: { id: string; path: string }) {
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
      <p role="status">{intl.formatMessage(m.peLoading)}</p>
    )
  }
  const read = readEstimates(file.data.content)
  if ('refusal' in read)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.peUnreadable, { reason: read.refusal })}
      </p>
    )
  return (
    <Editor
      key={`${file.data.revision}:${file.dataUpdatedAt}`}
      id={id}
      path={path}
      draft={draft.data}
      file={file.data}
      estimates={read.estimates}
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
  estimates: Estimates
  onReload: () => Promise<void>
}

function Editor({ id, path, draft, file, estimates, onReload }: EditorProps) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { heading: useId(), types: useId(), import: useId(), file: useId(), csv: useId(), districts: useId() }
  const [csv, setCsv] = useState('')
  const [revision, setRevision] = useState(file.revision)
  const editable = draft.editable
  const title = (code: string) => estimates.types.find((t) => t.code === code)?.title ?? code
  const places = new Set(estimates.figures.map((f) => f.place)).size
  const check = useMutation({ mutationFn: (): Promise<Checked> => checkCsv(platform.transport, estimates, csv) })
  const checked = check.data?.kind === 'checked' ? check.data : null
  const save = useMutation({
    mutationFn: () => {
      if (checked === null) throw new Error('nothing checked')
      return saveFile(platform.transport, id, path, `${JSON.stringify(checked.estimates, null, 2)}\n`, revision)
    },
    onSuccess: async (next) => {
      setRevision(next.revision)
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
  const stale = save.error instanceof StudioError && save.error.status === 412
  const reason = save.error instanceof StudioError ? (save.error.refusal === null || save.error.refusal === undefined ? save.error.detail : refusalText(intl, save.error.refusal)) : save.error === null ? '' : String(save.error)
  const href = `data:text/csv;charset=utf-8,${encodeURIComponent(template(estimates))}`
  return (
    <section className={page} aria-labelledby={ids.heading}>
      <h1 id={ids.heading} className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
        {intl.formatMessage(m.peHeading, { year: estimates.year })}
      </h1>
      {editable ? null : (
        <p className={muted}>{draft.editRefusal === null || draft.editRefusal === undefined ? intl.formatMessage(m.peReadOnly) : intl.formatMessage(sm.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}</p>
      )}
      <table className={grid}>
        <caption id={ids.types} className={css({ textAlign: 'left', fontWeight: 'semibold', fontSize: 'lg', pb: '1' })}>
          {intl.formatMessage(m.peTypes)}
        </caption>
        <thead>
          <tr>
            <th scope="col">{intl.formatMessage(m.peType)}</th>
            <th scope="col">{intl.formatMessage(m.peCode)}</th>
            <th scope="col">{intl.formatMessage(m.peHow)}</th>
          </tr>
        </thead>
        <tbody>
          {estimates.types.map((t) => (
            <tr key={t.code}>
              <td>{t.title ?? t.code}</td>
              <td>
                <code>{t.code}</code>
              </td>
              <td>{t.derivedFrom === undefined ? intl.formatMessage(m.peGiven) : intl.formatMessage(m.peDerived, { percent: t.percent ?? '', from: title(t.derivedFrom) })}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p role="status">{intl.formatMessage(m.peFigures, { count: estimates.figures.length, places })}</p>
      {editable ? (
        <form
          className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
          aria-labelledby={ids.import}
          onSubmit={(e) => {
            e.preventDefault()
            save.reset()
            check.mutate()
          }}
        >
          <h2 id={ids.import} className={subheading}>
            {intl.formatMessage(m.peImport)}
          </h2>
          <p className={muted}>{intl.formatMessage(m.peTemplateHint)}</p>
          <div>
            <a href={href} download={`population-${estimates.year}-template.csv`}>
              {intl.formatMessage(m.peTemplate)}
            </a>
            <span className={muted}>
              {' '}
              ({['place', ...givenTypes(estimates).map((t) => t.code)].join(', ')})
            </span>
          </div>
          <div className={field}>
            <label htmlFor={ids.file}>{intl.formatMessage(m.peFile)}</label>
            <input
              id={ids.file}
              type="file"
              accept=".csv,text/csv"
              onChange={(e) => {
                const chosen = e.target.files?.[0]
                if (chosen !== undefined) void chosen.text().then(setCsv)
              }}
            />
          </div>
          <div className={field}>
            <label htmlFor={ids.csv}>{intl.formatMessage(m.peCsv)}</label>
            <textarea id={ids.csv} className={`${control} ${area}`} value={csv} spellCheck={false} onChange={(e) => setCsv(e.target.value)} />
          </div>
          <div className={actions}>
            <button type="submit" className={button} disabled={csv.trim() === '' || check.isPending}>
              {intl.formatMessage(m.peCheck)}
            </button>
            {check.isPending ? <span role="status">{intl.formatMessage(m.peChecking)}</span> : null}
          </div>
        </form>
      ) : null}
      {check.data?.kind === 'refused' ? (
        <div role="alert">
          <p className={alert}>{intl.formatMessage(m.peRefused, { count: check.data.problems.length })}</p>
          <ul className={css({ pl: '5', listStyle: 'disc' })}>
            {check.data.problems.map((p, i) => (
              <li key={i} className={alert}>
                {p}
              </li>
            ))}
          </ul>
        </div>
      ) : check.data?.kind === 'failed' ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.peCheckFailed, { reason: check.data.detail === '' ? String(check.data.status) : check.data.detail })}
        </p>
      ) : check.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.peCheckFailed, { reason: String(check.error) })}
        </p>
      ) : null}
      {checked === null ? null : (
        <>
          <p role="status">{intl.formatMessage(m.peFigures, { count: checked.figures, places: new Set(checked.estimates.figures.map((f) => f.place)).size })}</p>
          <table className={grid}>
            <caption id={ids.districts} className={css({ textAlign: 'left', fontWeight: 'semibold', fontSize: 'lg', pb: '1' })}>
              {intl.formatMessage(m.peDistricts, { count: checked.districts.length })}
            </caption>
            <thead>
              <tr>
                <th scope="col">{intl.formatMessage(m.peDistrict)}</th>
                {estimates.types.map((t) => (
                  <th key={t.code} scope="col" className={numeric}>
                    {t.title ?? t.code}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {checked.districts.map((d) => (
                <tr key={d.code}>
                  <th scope="row">{d.name}</th>
                  {estimates.types.map((t) => {
                    const annual = d.totals.find((x) => x.type === t.code)?.annual ?? null
                    return (
                      <td key={t.code} className={numeric}>
                        {annual === null ? <span className={muted}>{intl.formatMessage(m.peNoEstimate)}</span> : grouped(annual, intl.locale)}
                      </td>
                    )
                  })}
                </tr>
              ))}
            </tbody>
          </table>
          {checked.problems.length === 0 ? null : (
            <div role="alert">
              <p className={alert}>{intl.formatMessage(m.peProblems, { count: checked.problems.length })}</p>
              <ul className={css({ pl: '5', listStyle: 'disc' })}>
                {checked.problems.map((p, i) => (
                  <li key={i} className={alert}>
                    {p}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div className={actions}>
            <button type="button" className={button} disabled={save.isPending || stale || save.isSuccess} onClick={() => save.mutate()}>
              {intl.formatMessage(m.peSave)}
            </button>
            {save.isSuccess && !stale ? <span role="status">{intl.formatMessage(m.peSaved)}</span> : null}
          </div>
        </>
      )}
      {stale ? (
        <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
          <p className={alert}>{intl.formatMessage(m.peStale)}</p>
          <button type="button" className={button} onClick={() => void onReload()}>
            {intl.formatMessage(m.peReload)}
          </button>
        </div>
      ) : save.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.peFailed, { reason })}
        </p>
      ) : null}
    </section>
  )
}
