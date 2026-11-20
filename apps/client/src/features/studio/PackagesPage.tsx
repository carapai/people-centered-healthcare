import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { studioMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button, control, field } from '../reception/styles'
import { StudioError, packageQuery, packagesQuery, startDraft, studioKey, type StudioPackage, type StudioPackageKind } from './api'
import { actionLabel, refusalText, stageLabel } from './wording'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '6xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })

/**
 * The Studio's package list (studio.md § The package workspace; #492): one row per package with
 * its title, name and kind, its version and bump, its status and Draft Holder, the signed-in person's
 * next step, and the scopes it is active in. The server works out the next step.
 */
export function PackagesPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const navigate = useNavigate()
  const packages = useQuery(packagesQuery(platform.transport))
  const [view, setView] = useState<DataTableView>(emptyView)
  const open = (row: StudioPackage) => void navigate({ to: '/studio/$id', params: { id: row.id } })
  const columns: DataTableColumn<StudioPackage>[] = [
    {
      id: 'package',
      header: intl.formatMessage(m.sdColPackage),
      cell: (row) => `${row.title ?? row.name} · ${row.name} · ${row.kind}`,
      sortValue: (row) => row.title ?? row.name,
    },
    {
      id: 'version',
      header: intl.formatMessage(m.sdColVersion),
      cell: (row) =>
        row.bump === null || row.bump === undefined
          ? intl.formatMessage(m.sdVersionFirst, { version: row.version })
          : intl.formatMessage(m.sdVersionBump, { version: row.version, bump: row.bump }),
    },
    { id: 'status', header: intl.formatMessage(m.sdColStatus), cell: (row) => stageLabel(intl, row.stage), sortValue: (row) => row.stage },
    { id: 'author', header: intl.formatMessage(m.sdColAuthor), cell: (row) => row.holder },
    {
      id: 'next',
      header: intl.formatMessage(m.sdColNext),
      cell: (row) => (row.next === null || row.next === undefined ? intl.formatMessage(m.sdNothing) : actionLabel(intl, row.next)),
    },
    {
      id: 'active',
      header: intl.formatMessage(m.sdColActive),
      cell: (row) => (row.active.length === 0 ? intl.formatMessage(m.sdNowhere) : row.active.map((a) => intl.formatMessage(m.sdActiveIn, a)).join(', ')),
    },
  ]
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.sdTitle)}</h1>
      {packages.data === undefined ? (
        packages.isPending ? (
          <p role="status">{intl.formatMessage(m.sdLoading)}</p>
        ) : (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(packages.error instanceof StudioError && packages.error.status === 403 ? m.sdRefused : m.sdFailed)}
          </p>
        )
      ) : packages.data.length === 0 ? (
        <p role="status">{intl.formatMessage(m.sdEmpty)}</p>
      ) : (
        <>
          <p className={css({ color: 'fg.muted' })}>{intl.formatMessage(m.sdHint)}</p>
          <DataTable caption={intl.formatMessage(m.sdTitle)} columns={columns} data={packages.data} getRowId={(row) => row.id} view={view} onViewChange={setView} onRowActivate={open} />
        </>
      )}
      {packages.data === undefined ? null : <NewPackage />}
    </section>
  )
}

const kinds: StudioPackageKind[] = ['program', 'national-core', 'terminology', 'country-configuration', 'override', 'language']

/** A blank draft of a new package (#494). The next version of a published package starts from
 * its page instead. */
function NewPackage() {
  const intl = useIntl()
  const platform = usePlatform()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const ids = { name: useId(), title: useId(), kind: useId(), version: useId(), canonical: useId() }
  const [form, setForm] = useState({ name: '', title: '', kind: 'program' as StudioPackageKind, version: '0.1.0', canonical: '' })
  const start = useMutation({
    mutationFn: () => startDraft(platform.transport, form),
    onSuccess: async (next) => {
      queryClient.setQueryData(packageQuery(platform.transport, next.package.id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
      await navigate({ to: '/studio/$id', params: { id: next.package.id } })
    },
  })
  const failure = start.error instanceof StudioError ? (start.error.refusal === null || start.error.refusal === undefined ? start.error.detail : refusalText(intl, start.error.refusal)) : ''
  const text = (key: 'name' | 'title' | 'version' | 'canonical', label: string) => (
    <div className={field}>
      <label htmlFor={ids[key]}>{label}</label>
      <input id={ids[key]} className={control} value={form[key]} required onChange={(e) => setForm({ ...form, [key]: e.target.value })} />
    </div>
  )
  return (
    <details>
      <summary className={css({ cursor: 'pointer', fontWeight: 'semibold' })}>{intl.formatMessage(m.sdNewDraft)}</summary>
      <form
        className={css({ display: 'flex', flexDirection: 'column', gap: '3', pt: '3' })}
        onSubmit={(e) => {
          e.preventDefault()
          start.mutate()
        }}
      >
        {text('name', intl.formatMessage(m.sdNewDraftName))}
        {text('title', intl.formatMessage(m.sdNewDraftTitle))}
        <div className={field}>
          <label htmlFor={ids.kind}>{intl.formatMessage(m.sdNewDraftKind)}</label>
          <select id={ids.kind} className={control} value={form.kind} onChange={(e) => setForm({ ...form, kind: e.target.value as StudioPackageKind })}>
            {kinds.map((kind) => (
              <option key={kind} value={kind}>
                {kind}
              </option>
            ))}
          </select>
        </div>
        {text('version', intl.formatMessage(m.sdNewDraftVersion))}
        {text('canonical', intl.formatMessage(m.sdNewDraftCanonical))}
        <div>
          <button type="submit" className={button} disabled={start.isPending}>
            {intl.formatMessage(m.sdNewDraftStart)}
          </button>
        </div>
        {start.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(m.sdStartFailed, { reason: failure })}
          </p>
        ) : null}
      </form>
    </details>
  )
}
