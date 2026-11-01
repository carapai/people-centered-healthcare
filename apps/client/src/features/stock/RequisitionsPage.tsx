import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { DEFAULT_STORE } from './StockPage'
import { StockError, decideRequisition, fulfilRequisition, onHandQuery, requestStock, requisitionsQuery, type Requisition } from './api'

const button = css({
  minHeight: 'control',
  px: 'controlX',
  bg: 'accent.default',
  color: 'accent.fg',
  borderRadius: 'md',
  fontSize: 'control',
  cursor: 'pointer',
  alignSelf: 'flex-start',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})
const card = css({ display: 'flex', flexDirection: 'column', gap: '2', p: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md' })

const statusLabels = {
  requested: messages.rqRequested,
  approved: messages.rqApproved,
  fulfilled: messages.rqFulfilled,
  rejected: messages.rqRejected,
} as const

/**
 * Ward requisitions (inventory-and-procurement.md § Receiving, requisitions and reorder; #250):
 * a ward asks the pharmacy for products; the pharmacy approves or refuses, then fulfils by
 * picking batches, which leave as a transfer to the ward. The server checks everything.
 */
export function RequisitionsPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const [store, setStore] = useState('Location/ward')
  const [supplier, setSupplier] = useState(DEFAULT_STORE)
  const [product, setProduct] = useState('')
  const [quantity, setQuantity] = useState('')
  const list = useQuery(requisitionsQuery(platform.transport, store))
  const refresh = () => queryClient.invalidateQueries({ queryKey: ['stock'] })
  const request = useMutation({
    mutationFn: () => requestStock(platform.transport, store, supplier, [{ product: product.trim(), quantity: Number(quantity) }]),
    onSuccess: () => {
      setProduct('')
      setQuantity('')
    },
    onSettled: refresh,
  })
  const decide = useMutation({ mutationFn: ({ id, approve }: { id: string; approve: boolean }) => decideRequisition(platform.transport, id, approve), onSettled: refresh })
  const canRequest = product.trim() !== '' && Number(quantity) > 0 && store.trim() !== '' && supplier.trim() !== '' && store !== supplier

  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.rqTitle)}</h1>
      <Field.Root>
        <Field.Label>{intl.formatMessage(messages.rqThisStore)}</Field.Label>
        <Field.Input value={store} onChange={(event) => setStore(event.target.value)} autoComplete="off" />
      </Field.Root>

      <form
        aria-labelledby="new-requisition"
        className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (canRequest) request.mutate()
        }}
      >
        <h2 id="new-requisition" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
          {intl.formatMessage(messages.rqNew)}
        </h2>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.rqFrom)}</Field.Label>
          <Field.Input value={supplier} onChange={(event) => setSupplier(event.target.value)} autoComplete="off" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.stProduct)}</Field.Label>
          <Field.Input value={product} onChange={(event) => setProduct(event.target.value)} autoComplete="off" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.stQuantity)}</Field.Label>
          <Field.Input inputMode="numeric" value={quantity} onChange={(event) => setQuantity(event.target.value)} />
        </Field.Root>
        <button type="submit" className={button} disabled={!canRequest || request.isPending}>
          {intl.formatMessage(messages.rqSend)}
        </button>
        {request.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(messages.rqFailed)} {request.error instanceof StockError ? request.error.reason : ''}
          </p>
        ) : null}
      </form>

      <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(messages.rqList)}</h2>
      {list.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.rqLoadFailed)}
        </p>
      ) : list.data === undefined ? (
        <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
      ) : list.data.length === 0 ? (
        <p role="status">{intl.formatMessage(messages.rqNone)}</p>
      ) : (
        <ul className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          {list.data.map((r) => (
            <li key={r.id} className={card} aria-label={`${r.requesting_store} → ${r.supplying_store}`}>
              <p>
                <strong>{r.requesting_store}</strong> → {r.supplying_store} · {intl.formatMessage(statusLabels[r.status])}
              </p>
              <p>{r.lines.map((l) => `${l.product} × ${String(l.quantity)}`).join(', ')}</p>
              {r.status === 'requested' && r.supplying_store === store ? (
                <div className={css({ display: 'flex', gap: '3' })}>
                  <button type="button" className={button} disabled={decide.isPending} onClick={() => decide.mutate({ id: r.id, approve: true })}>
                    {intl.formatMessage(messages.rqApprove)}
                  </button>
                  <button type="button" className={button} disabled={decide.isPending} onClick={() => decide.mutate({ id: r.id, approve: false })}>
                    {intl.formatMessage(messages.rqReject)}
                  </button>
                </div>
              ) : null}
              {r.status === 'approved' && r.supplying_store === store ? <Fulfil requisition={r} onDone={refresh} /> : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

/** Picking a batch for each line of an approved requisition, from the supplying store's stock. */
function Fulfil({ requisition, onDone }: { requisition: Requisition; onDone: () => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const stock = useQuery(onHandQuery(platform.transport, requisition.supplying_store))
  const [chosen, setChosen] = useState<Record<number, string>>({})
  const fulfil = useMutation({
    mutationFn: () =>
      fulfilRequisition(
        platform.transport,
        requisition.id,
        requisition.lines.map((line, i) => {
          const batch = (stock.data ?? []).find((s) => s.status === 'usable' && s.product === line.product && s.batch === chosen[i])
          return { product: line.product, batch: batch?.batch ?? '', expiry: batch?.expiry ?? '', funding_source: batch?.funding_source ?? 'government', quantity: Number(line.quantity) }
        }),
      ),
    onSettled: onDone,
  })
  const ready = requisition.lines.every((_, i) => (chosen[i] ?? '') !== '')
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      {requisition.lines.map((line, i) => (
        <Field.Root key={`${line.product}-${i}`} required>
          <Field.Label>
            {intl.formatMessage(messages.rqBatchFor)} {line.product}
          </Field.Label>
          <Field.Select value={chosen[i] ?? ''} onChange={(event) => setChosen({ ...chosen, [i]: event.target.value })}>
            <option value="">{intl.formatMessage(messages.rqChooseBatch)}</option>
            {(stock.data ?? [])
              .filter((s) => s.status === 'usable' && s.product === line.product && Number(s.available) > 0)
              .map((s) => (
                <option key={`${s.batch}|${s.funding_source}`} value={s.batch}>
                  {s.batch} · {s.expiry} · {String(s.available)}
                </option>
              ))}
          </Field.Select>
        </Field.Root>
      ))}
      <button type="button" className={button} disabled={!ready || fulfil.isPending} onClick={() => fulfil.mutate()}>
        {intl.formatMessage(messages.rqFulfil)}
      </button>
      {fulfil.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.rqFulfilFailed)} {fulfil.error instanceof StockError ? fulfil.error.reason : ''}
        </p>
      ) : null}
    </div>
  )
}
