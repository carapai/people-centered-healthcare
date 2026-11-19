import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { studioMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { StudioError, packagesQuery, type StudioPackage } from './api'
import { actionLabel, stageLabel } from './wording'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '6xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })

/**
 * The Studio's package list (studio.md § The package workspace; #492): one row per package with
 * its title, name and kind, its version and bump, its status and author, the signed-in person's
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
    { id: 'author', header: intl.formatMessage(m.sdColAuthor), cell: (row) => row.authors.join(', ') },
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
    </section>
  )
}
