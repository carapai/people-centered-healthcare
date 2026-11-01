import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { DEFAULT_STORE } from './StockPage'
import { StockError, lineComplete, readGs1, receiveGoods, type ReceivedLine } from './api'

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

interface LineForm {
  product: string
  batch: string
  expiry: string
  quantity: string
  packSize: string
  unitCost: string
  barcode: string
}

const blank: LineForm = { product: '', batch: '', expiry: '', quantity: '', packSize: '1', unitCost: '', barcode: '' }

const toLine = (f: LineForm): ReceivedLine => ({
  product: f.product,
  batch: f.batch,
  expiry: f.expiry,
  quantity: Number(f.quantity),
  pack_size: Number(f.packSize),
  unit_cost: f.unitCost,
})

/**
 * A goods received note (inventory-and-procurement.md § Receiving; #130): the supplier and their
 * delivery number, and a line per batch, each received at its unit cost. A GS1 barcode, where
 * there is one, fills a line's batch and expiry; it is never required.
 */
export function ReceivePage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const [store, setStore] = useState(DEFAULT_STORE)
  const [supplier, setSupplier] = useState('NMS')
  const [delivery, setDelivery] = useState('')
  const [lines, setLines] = useState<LineForm[]>([blank])
  const [done, setDone] = useState<string | undefined>()
  const scan = useMutation({
    mutationFn: ({ index, data }: { index: number; data: string }) => readGs1(platform.transport, data).then((read) => ({ index, read })),
    onSuccess: ({ index, read }) =>
      setLines((current) =>
        current.map((line, i) => (i === index ? { ...line, batch: read.batch ?? line.batch, expiry: read.expiry ?? line.expiry } : line)),
      ),
  })
  const receive = useMutation({
    mutationFn: () => receiveGoods(platform.transport, { store, supplier, delivery, lines: lines.map(toLine) }),
    onSuccess: (id) => {
      setDone(id)
      void queryClient.invalidateQueries({ queryKey: ['stock'] })
    },
  })
  const complete = supplier.trim() !== '' && delivery.trim() !== '' && lines.every((l) => lineComplete(toLine(l)))
  const set = (index: number, field: keyof LineForm) => (event: { target: { value: string } }) =>
    setLines((current) => current.map((line, i) => (i === index ? { ...line, [field]: event.target.value } : line)))

  if (done !== undefined) {
    return (
      <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.stReceive)}</h1>
        <p role="status">{intl.formatMessage(messages.stReceived)}</p>
        <Link to="/stock">{intl.formatMessage(messages.stTitle)}</Link>
      </section>
    )
  }
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '3xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.stReceive)}</h1>
      <form
        className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (complete) receive.mutate()
        }}
      >
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.stStore)}</Field.Label>
          <Field.Input value={store} onChange={(event) => setStore(event.target.value)} autoComplete="off" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.stSupplier)}</Field.Label>
          <Field.Input value={supplier} onChange={(event) => setSupplier(event.target.value)} autoComplete="off" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.stDelivery)}</Field.Label>
          <Field.Input value={delivery} onChange={(event) => setDelivery(event.target.value)} autoComplete="off" />
        </Field.Root>
        {lines.map((line, index) => (
          <fieldset key={index} className={css({ display: 'flex', flexDirection: 'column', gap: '3', p: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md' })}>
            <legend>
              {intl.formatMessage(messages.stLine)} {index + 1}
            </legend>
            <Field.Root>
              <Field.Label>{intl.formatMessage(messages.stBarcode)}</Field.Label>
              <Field.Input value={line.barcode} onChange={set(index, 'barcode')} autoComplete="off" />
              <Field.HelperText>{intl.formatMessage(messages.stBarcodeHelp)}</Field.HelperText>
            </Field.Root>
            <button
              type="button"
              className={button}
              disabled={line.barcode.trim() === '' || scan.isPending}
              onClick={() => scan.mutate({ index, data: line.barcode })}
            >
              {intl.formatMessage(messages.stReadBarcode)}
            </button>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.stProduct)}</Field.Label>
              <Field.Input value={line.product} onChange={set(index, 'product')} autoComplete="off" />
            </Field.Root>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.stBatch)}</Field.Label>
              <Field.Input value={line.batch} onChange={set(index, 'batch')} autoComplete="off" />
            </Field.Root>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.stExpiry)}</Field.Label>
              <Field.Input type="date" value={line.expiry} onChange={set(index, 'expiry')} />
            </Field.Root>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.stQuantity)}</Field.Label>
              <Field.Input inputMode="numeric" value={line.quantity} onChange={set(index, 'quantity')} />
            </Field.Root>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.stPackSize)}</Field.Label>
              <Field.Input inputMode="numeric" value={line.packSize} onChange={set(index, 'packSize')} />
            </Field.Root>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.stUnitCost)}</Field.Label>
              <Field.Input inputMode="decimal" value={line.unitCost} onChange={set(index, 'unitCost')} />
            </Field.Root>
          </fieldset>
        ))}
        {scan.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(messages.stBarcodeFailed)}
          </p>
        ) : null}
        <button type="button" className={button} onClick={() => setLines([...lines, blank])}>
          {intl.formatMessage(messages.stAddLine)}
        </button>
        <button type="submit" className={button} disabled={!complete || receive.isPending}>
          {intl.formatMessage(messages.stReceiveSubmit)}
        </button>
        {receive.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(messages.stReceiveFailed)} {receive.error instanceof StockError ? receive.error.reason : ''}
          </p>
        ) : null}
      </form>
    </section>
  )
}
