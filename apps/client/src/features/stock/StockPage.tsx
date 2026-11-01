import { DataTable, Field, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { onHandQuery, reorderQuery, type OnHand, type Suggestion } from './api'

export const DEFAULT_STORE = 'Location/pharmacy'

/**
 * A store's stock on hand and its reorder suggestions (inventory-and-procurement.md; #130): each
 * batch's balance, what is available, and its last unit cost; and per product, the min/max
 * levels from the last 30 days' consumption and what to order.
 */
export function StockPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const [store, setStore] = useState(DEFAULT_STORE)
  const onHand = useQuery(onHandQuery(platform.transport, store))
  const reorder = useQuery(reorderQuery(platform.transport, store))
  const [view, setView] = useState<DataTableView>(emptyView)
  const [reorderView, setReorderView] = useState<DataTableView>(emptyView)
  const columns: DataTableColumn<OnHand>[] = [
    { id: 'product', header: intl.formatMessage(messages.stProduct), cell: (r) => r.product, sortValue: (r) => r.product },
    { id: 'batch', header: intl.formatMessage(messages.stBatch), cell: (r) => r.batch },
    { id: 'expiry', header: intl.formatMessage(messages.stExpiry), cell: (r) => r.expiry, sortValue: (r) => r.expiry },
    { id: 'status', header: intl.formatMessage(messages.stStatus), cell: (r) => r.status },
    { id: 'onHand', header: intl.formatMessage(messages.stOnHand), cell: (r) => String(r.on_hand), sortValue: (r) => r.on_hand },
    { id: 'cost', header: intl.formatMessage(messages.stUnitCost), cell: (r) => r.unit_cost ?? '' },
  ]
  const reorderColumns: DataTableColumn<Suggestion>[] = [
    { id: 'product', header: intl.formatMessage(messages.stProduct), cell: (r) => r.product, sortValue: (r) => r.product },
    { id: 'onHand', header: intl.formatMessage(messages.stOnHand), cell: (r) => String(r.on_hand) },
    { id: 'monthly', header: intl.formatMessage(messages.stMonthly), cell: (r) => r.monthly },
    { id: 'min', header: intl.formatMessage(messages.stMin), cell: (r) => String(r.min) },
    { id: 'max', header: intl.formatMessage(messages.stMax), cell: (r) => String(r.max) },
    { id: 'order', header: intl.formatMessage(messages.stOrder), cell: (r) => String(r.order), sortValue: (r) => r.order },
  ]
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '5xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.stTitle)}</h1>
      <Field.Root>
        <Field.Label>{intl.formatMessage(messages.stStore)}</Field.Label>
        <Field.Input value={store} onChange={(event) => setStore(event.target.value)} autoComplete="off" />
      </Field.Root>
      <Link to="/stock/receive">{intl.formatMessage(messages.stReceive)}</Link>
      {onHand.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.stFailed)}
        </p>
      ) : onHand.data === undefined ? (
        <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
      ) : onHand.data.length === 0 ? (
        <p role="status">{intl.formatMessage(messages.stEmpty)}</p>
      ) : (
        <DataTable
          caption={intl.formatMessage(messages.stOnHandCaption)}
          columns={columns}
          data={onHand.data}
          getRowId={(r) => `${r.product}|${r.batch}|${r.status}|${r.funding_source}`}
          view={view}
          onViewChange={setView}
          height="18rem"
        />
      )}
      <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(messages.stReorder)}</h2>
      <p>{intl.formatMessage(messages.stReorderHint)}</p>
      {reorder.data === undefined || reorder.data.length === 0 ? null : (
        <DataTable
          caption={intl.formatMessage(messages.stReorder)}
          columns={reorderColumns}
          data={reorder.data}
          getRowId={(r) => r.product}
          view={reorderView}
          onViewChange={setReorderView}
          height="14rem"
        />
      )}
    </section>
  )
}
