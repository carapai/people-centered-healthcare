import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import type { Grant } from './api'

/** The grants in a DataTable: when, patient, user and justification, then `extra` columns. */
export function GrantTable({
  caption,
  grants,
  extra,
  onRowActivate,
}: {
  caption: string
  grants: readonly Grant[]
  extra: readonly DataTableColumn<Grant>[]
  onRowActivate?: (grant: Grant) => void
}) {
  const intl = useIntl()
  const [view, setView] = useState<DataTableView>(emptyView)
  const columns: DataTableColumn<Grant>[] = [
    { id: 'at', header: intl.formatMessage(messages.bgColWhen), cell: (g) => formatTime(intl, g.at), sortValue: (g) => g.at },
    { id: 'patient', header: intl.formatMessage(messages.bgColPatient), cell: (g) => g.patient, sortValue: (g) => g.patient },
    { id: 'account', header: intl.formatMessage(messages.bgColUser), cell: (g) => g.account, sortValue: (g) => g.account },
    { id: 'justification', header: intl.formatMessage(messages.bgColReason), cell: (g) => g.justification, canHide: true },
    ...extra,
  ]
  return (
    <DataTable
      caption={caption}
      columns={columns}
      data={grants}
      getRowId={(g) => g.id}
      view={view}
      onViewChange={setView}
      onRowActivate={onRowActivate}
      height="24rem"
    />
  )
}

/** An instant as a localized date and time. */
export function formatTime(intl: ReturnType<typeof useIntl>, iso: string): string {
  return intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
}
