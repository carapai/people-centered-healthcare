import { css, cx } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useId, useRef, useState } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { ageBandMessages as m, studioMessages as sm } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button, control, field } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { icu } from '../designs'
import { refusalText } from '../wording'
import { ageAts, ageUnits, bandLabel, bandProblems, flagged, nextVersion, readAgeBands, writeAgeBands, type AgeAt, type AgeBandTable, type AgeUnit, type BandProblem } from './ageBands'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })
const muted = css({ color: 'fg.muted' })
const alert = css({ color: 'danger.text' })
const settings = css({ display: 'grid', gap: '3', gridTemplateColumns: { base: '1fr', md: '2fr 1fr 2fr' } })
const grid = css({ width: '100%', borderCollapse: 'collapse', '& th, & td': { textAlign: 'left', p: '2', borderBottom: '1px solid', borderColor: 'border' } })
const flaggedRow = css({ bg: 'danger.subtle', outline: '2px solid', outlineColor: 'danger.border', outlineOffset: '-2px' })
const number = cx(control, css({ width: '7rem' }))
const actions = css({ display: 'flex', flexWrap: 'wrap', gap: '2', alignItems: 'center' })
const secondary = css({
  minHeight: 'control',
  px: '3',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})

/**
 * An Age Band Set as a small table (analytics.md § Age Band Sets and Population Estimates; ADR
 * 0071; #551): its name, the unit, the date age is taken at, and its from–to rows, with gaps and
 * overlaps flagged on the rows as the author types. *Version* gives the set's own version against
 * the latest published one (a band change is major, a rename a patch) and saving writes it. Saving
 * goes through the file editor's `If-Match` path; a 412 keeps nothing and offers *Reload*.
 */
export function AgeBandSetEditor({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  const file = useQuery({ ...fileQuery(platform.transport, id, path), retry: false })
  const previousId = draft.data?.package.previous === null || draft.data?.package.previous === undefined ? null : `${draft.data.package.name}@${draft.data.package.previous}`
  const previous = useQuery({ ...fileQuery(platform.transport, previousId ?? '', path), enabled: previousId !== null, retry: false })
  // A previous version without this file published no version of it.
  const previousSettled = previousId === null || previous.data !== undefined || (previous.error instanceof StudioError && previous.error.status === 404)
  if (draft.data === undefined || file.data === undefined || !previousSettled) {
    const failed = draft.isError || file.isError || (previous.isError && !previousSettled)
    return failed ? (
      <p role="alert" className={alert}>
        {intl.formatMessage(sm.sdFailed)}
      </p>
    ) : (
      <p role="status">{intl.formatMessage(m.abLoading)}</p>
    )
  }
  const read = readAgeBands(file.data.content)
  if ('refusal' in read)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.abUnreadable, { reason: read.refusal })}
      </p>
    )
  const published = previous.data === undefined ? null : readAgeBands(previous.data.content)
  return (
    <Editor
      key={`${file.data.revision}:${file.dataUpdatedAt}`}
      id={id}
      path={path}
      draft={draft.data}
      file={file.data}
      library={read.library}
      initial={read.table}
      published={published === null || 'refusal' in published ? null : { version: String(published.library['version'] ?? ''), table: published.table }}
      onReload={async () => {
        await Promise.all([draft.refetch(), file.refetch(), previous.refetch()])
      }}
    />
  )
}

/** One problem, in words. */
function problemText(intl: IntlShape, problem: BandProblem, table: AgeBandTable): string {
  const unit = intl.formatMessage(m.abUnitName, { unit: table.unit }).toLowerCase()
  const label = (row: number) => bandLabel(table.bands[row] ?? { from: null, to: null }, table.unit)
  switch (problem.kind) {
    case 'no-title':
      return intl.formatMessage(m.abNoTitle)
    case 'no-bands':
      return intl.formatMessage(m.abNoBands)
    case 'no-from':
      return intl.formatMessage(m.abNoFrom, { row: problem.rows[0] + 1 })
    case 'negative':
      return intl.formatMessage(m.abNegative, { row: problem.rows[0] + 1 })
    case 'goes-down':
      return intl.formatMessage(m.abGoesDown, { row: problem.rows[0] + 1 })
    case 'gap':
      return intl.formatMessage(m.abGap, {
        first: label(problem.rows[0]),
        second: label(problem.rows[1]),
        ages: problem.to === problem.from ? intl.formatMessage(m.abAge, { age: problem.from, unit }) : intl.formatMessage(m.abAges, { from: problem.from, to: problem.to, unit }),
      })
    case 'overlap':
      return intl.formatMessage(m.abOverlap, {
        first: label(problem.rows[0]),
        second: label(problem.rows[1]),
        ages:
          problem.to === null
            ? intl.formatMessage(m.abAgesOver, { from: problem.from, unit })
            : problem.to === problem.from
              ? intl.formatMessage(m.abAge, { age: problem.from, unit })
              : intl.formatMessage(m.abAges, { from: problem.from, to: problem.to, unit }),
      })
  }
}

/** A whole number from a field, or null when it is empty or not one. */
const whole = (text: string): number | null => (/^-?\d+$/.test(text.trim()) ? Number.parseInt(text, 10) : null)

interface EditorProps {
  id: string
  path: string
  draft: StudioPackagePage
  file: StudioFile
  library: Record<string, unknown>
  initial: AgeBandTable
  published: { version: string; table: AgeBandTable } | null
  onReload: () => Promise<void>
}

function Editor({ id, path, draft, file, library, initial, published, onReload }: EditorProps) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { heading: useId(), title: useId(), unit: useId(), ageAt: useId(), bands: useId(), hint: useId(), problems: useId() }
  const [table, setTable] = useState(initial)
  const [revision, setRevision] = useState(file.revision)
  const rowsRef = useRef<HTMLTableSectionElement>(null)
  // After *Add a band*, the new row's From field takes the focus.
  const [added, setAdded] = useState(false)
  useEffect(() => {
    if (!added) return
    rowsRef.current?.querySelector<HTMLInputElement>('tr:last-child input')?.focus()
    setAdded(false)
  }, [added])
  const editable = draft.editable
  const problems = bandProblems(table)
  const rows = flagged(problems)
  const current = typeof library['version'] === 'string' ? library['version'] : draft.package.version
  const version = nextVersion(published, current, table)
  const save = useMutation({
    mutationFn: () => saveFile(platform.transport, id, path, JSON.stringify(writeAgeBands(library, table, version.version), null, 2), revision),
    onSuccess: async (next) => {
      setRevision(next.revision)
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
  const stale = save.error instanceof StudioError && save.error.status === 412
  const reason = save.error instanceof StudioError ? (save.error.refusal === null || save.error.refusal === undefined ? save.error.detail : refusalText(intl, save.error.refusal)) : save.error === null ? '' : String(save.error)
  const change = (next: Partial<AgeBandTable>) => setTable((t) => ({ ...t, ...next }))
  const setBand = (row: number, next: { from?: number | null; to?: number | null }) => change({ bands: table.bands.map((b, i) => (i === row ? { ...b, ...next } : b)) })
  const move = (row: number, by: -1 | 1) => {
    const to = row + by
    if (to < 0 || to >= table.bands.length) return
    const bands = [...table.bands]
    ;[bands[row], bands[to]] = [bands[to]!, bands[row]!]
    change({ bands })
  }
  const add = () => {
    const last = table.bands.at(-1)
    const from = last === undefined ? 0 : last.to === null ? (last.from ?? 0) + 1 : last.to + 1
    change({ bands: [...table.bands, { from, to: null }] })
    setAdded(true)
  }
  return (
    <section className={page} aria-labelledby={ids.heading}>
      <h1 id={ids.heading} className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
        {table.title.trim() === '' ? path : table.title}
      </h1>
      {editable ? null : (
        <p className={muted}>{draft.editRefusal === null || draft.editRefusal === undefined ? intl.formatMessage(m.abReadOnly) : intl.formatMessage(sm.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}</p>
      )}
      <div className={settings}>
        <div className={field}>
          <label htmlFor={ids.title}>{intl.formatMessage(m.abTitle)}</label>
          <input id={ids.title} className={control} value={table.title} readOnly={!editable} onChange={(e) => change({ title: e.target.value })} />
        </div>
        <div className={field}>
          <label htmlFor={ids.unit}>{intl.formatMessage(m.abUnit)}</label>
          <select id={ids.unit} className={control} value={table.unit} disabled={!editable} onChange={(e) => change({ unit: e.target.value as AgeUnit })}>
            {ageUnits.map((unit) => (
              <option key={unit} value={unit}>
                {intl.formatMessage(m.abUnitName, { unit })}
              </option>
            ))}
          </select>
        </div>
        <div className={field}>
          <label htmlFor={ids.ageAt}>{intl.formatMessage(m.abAgeAt)}</label>
          <select id={ids.ageAt} className={control} value={table.ageAt} disabled={!editable} onChange={(e) => change({ ageAt: e.target.value as AgeAt })}>
            {ageAts.map((at) => (
              <option key={at} value={at}>
                {intl.formatMessage(m.abAgeAtName, { at: icu(at) })}
              </option>
            ))}
          </select>
        </div>
      </div>
      <table className={grid} aria-labelledby={ids.bands} aria-describedby={ids.hint}>
        <caption id={ids.bands} className={css({ textAlign: 'left', fontWeight: 'semibold', fontSize: 'lg', pb: '1' })}>
          {intl.formatMessage(m.abBands)}
        </caption>
        <thead>
          <tr>
            <th scope="col">{intl.formatMessage(m.abFrom)}</th>
            <th scope="col">{intl.formatMessage(m.abTo)}</th>
            <th scope="col">{intl.formatMessage(m.abBand)}</th>
            {editable ? <th scope="col">{intl.formatMessage(m.abActions)}</th> : null}
          </tr>
        </thead>
        <tbody ref={rowsRef}>
          {table.bands.map((band, row) => {
            const name = intl.formatMessage(m.abRow, { row: row + 1 })
            const bad = rows.has(row)
            return (
              <tr key={row} className={bad ? flaggedRow : undefined} aria-invalid={bad || undefined} data-flagged={bad || undefined}>
                <td>
                  <input
                    className={number}
                    inputMode="numeric"
                    aria-label={`${name}: ${intl.formatMessage(m.abFrom)}`}
                    value={band.from ?? ''}
                    readOnly={!editable}
                    onChange={(e) => setBand(row, { from: whole(e.target.value) })}
                  />
                </td>
                <td>
                  <input
                    className={number}
                    inputMode="numeric"
                    aria-label={`${name}: ${intl.formatMessage(m.abTo)}`}
                    placeholder={intl.formatMessage(m.abOpen)}
                    value={band.to ?? ''}
                    readOnly={!editable}
                    onChange={(e) => setBand(row, { to: whole(e.target.value) })}
                  />
                </td>
                <td>{bandLabel(band, table.unit)}</td>
                {editable ? (
                  <td>
                    <div className={actions}>
                      <button type="button" className={secondary} disabled={row === 0} aria-label={intl.formatMessage(m.abUp, { row: row + 1 })} onClick={() => move(row, -1)}>
                        ↑
                      </button>
                      <button type="button" className={secondary} disabled={row === table.bands.length - 1} aria-label={intl.formatMessage(m.abDown, { row: row + 1 })} onClick={() => move(row, 1)}>
                        ↓
                      </button>
                      <button type="button" className={secondary} aria-label={intl.formatMessage(m.abRemove, { row: row + 1 })} onClick={() => change({ bands: table.bands.filter((_, i) => i !== row) })}>
                        ✕
                      </button>
                    </div>
                  </td>
                ) : null}
              </tr>
            )
          })}
        </tbody>
      </table>
      <p id={ids.hint} className={muted}>
        {intl.formatMessage(m.abToHint)}
      </p>
      {editable ? (
        <div>
          <button type="button" className={secondary} onClick={add}>
            {intl.formatMessage(m.abAdd)}
          </button>
        </div>
      ) : null}
      <div aria-labelledby={ids.problems} role="region">
        <h2 id={ids.problems} className={css({ fontWeight: 'semibold' })}>
          {problems.length === 0 ? intl.formatMessage(m.abNoProblems) : intl.formatMessage(m.abProblems, { count: problems.length })}
        </h2>
        {problems.length === 0 ? null : (
          <ul className={css({ pl: '5', listStyle: 'disc' })}>
            {problems.map((p, i) => (
              <li key={i} className={alert}>
                {problemText(intl, p, table)}
              </li>
            ))}
          </ul>
        )}
        <p className={muted}>{intl.formatMessage(m.abAuthority)}</p>
      </div>
      <p role="status">{intl.formatMessage(m.abVersion, { version: version.version, reason: version.reason, published: published?.version ?? '' })}</p>
      {version.reason === 'bands' && editable ? <p className={muted}>{intl.formatMessage(m.abMajorPackage)}</p> : null}
      {editable ? (
        <div className={actions}>
          <button type="button" className={button} disabled={save.isPending || stale} onClick={() => save.mutate()}>
            {intl.formatMessage(m.abSave)}
          </button>
          {save.isSuccess && !stale ? <span role="status">{intl.formatMessage(m.abSaved)}</span> : null}
        </div>
      ) : null}
      {stale ? (
        <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
          <p className={alert}>{intl.formatMessage(m.abStale)}</p>
          <button type="button" className={button} onClick={() => void onReload()}>
            {intl.formatMessage(m.abReload)}
          </button>
        </div>
      ) : save.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.abFailed, { reason })}
        </p>
      ) : null}
    </section>
  )
}

