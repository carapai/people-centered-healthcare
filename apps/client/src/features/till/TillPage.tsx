import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { METHODS, TillError, closeShift, openShift, openShiftQuery, shiftKey, takePayment, type PaymentMethod } from './api'

export const button = css({
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

export const methodLabels = {
  cash: messages.tlCash,
  'mtn-momo': messages.tlMtn,
  'airtel-money': messages.tlAirtel,
  card: messages.tlCard,
  'bank-transfer': messages.tlBank,
  'customer-reported': messages.tlReported,
} as const

const receiptLabels = { provisional: messages.tlProvisional, confirmed: messages.tlConfirmed, void: messages.tlVoid } as const

const reasonOf = (error: unknown) => (error instanceof TillError ? error.reason : '')

/**
 * The cashier's till (billing-and-payments.md § Payments; ADR 0032; #254): open a shift, take
 * cash or record a mobile-money, card or bank payment with its provider reference, and close
 * with the cash counted. A non-cash receipt says "provisional" until the provider confirms it.
 */
export function TillPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const [till, setTill] = useState('till-1')
  const [float, setFloat] = useState('0')
  const [method, setMethod] = useState<PaymentMethod>('cash')
  const [amount, setAmount] = useState('')
  const [payer, setPayer] = useState('')
  const [reference, setReference] = useState('')
  const [counted, setCounted] = useState('')
  const current = useQuery(openShiftQuery(platform.transport, till))
  const refresh = () => queryClient.invalidateQueries({ queryKey: shiftKey(till) })
  const open = useMutation({ mutationFn: () => openShift(platform.transport, till, float), onSettled: refresh })
  const take = useMutation({
    mutationFn: (shift: string) => takePayment(platform.transport, shift, method, amount, payer, reference),
    onSuccess: () => {
      setAmount('')
      setReference('')
    },
    onSettled: refresh,
  })
  const close = useMutation({ mutationFn: (shift: string) => closeShift(platform.transport, shift, counted), onSettled: refresh })
  const canTake = Number(amount) > 0 && payer.trim() !== '' && (method === 'cash' || reference.trim() !== '')
  const shift = current.data?.shift

  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '3xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.tlTitle)}</h1>
      <Field.Root>
        <Field.Label>{intl.formatMessage(messages.tlTill)}</Field.Label>
        <Field.Input value={till} onChange={(event) => setTill(event.target.value)} autoComplete="off" />
      </Field.Root>
      <Link to="/till/supervise">{intl.formatMessage(messages.tlSuperviseTitle)}</Link>
      {current.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.tlFailed)} {reasonOf(current.error)}
        </p>
      ) : current.isPending ? (
        <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
      ) : shift === undefined ? (
        <form
          className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
          onSubmit={(event) => {
            event.preventDefault()
            open.mutate()
          }}
        >
          <p role="status">{intl.formatMessage(messages.tlClosed)}</p>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.tlFloat)}</Field.Label>
            <Field.Input inputMode="decimal" value={float} onChange={(event) => setFloat(event.target.value)} />
          </Field.Root>
          <button type="submit" className={button} disabled={open.isPending}>
            {intl.formatMessage(messages.tlOpen)}
          </button>
          {open.isError ? <p role="alert">{reasonOf(open.error)}</p> : null}
        </form>
      ) : (
        <>
          <p role="status">
            {intl.formatMessage(messages.tlShift)} {shift.id} · {intl.formatMessage(messages.tlExpected)} {shift.expected}
          </p>
          <form
            aria-labelledby="take-payment"
            className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
            onSubmit={(event) => {
              event.preventDefault()
              if (canTake) take.mutate(shift.id)
            }}
          >
            <h2 id="take-payment" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
              {intl.formatMessage(messages.tlTake)}
            </h2>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.tlMethod)}</Field.Label>
              <Field.Select value={method} onChange={(event) => setMethod(event.target.value as PaymentMethod)}>
                {METHODS.map((m) => (
                  <option key={m} value={m}>
                    {intl.formatMessage(methodLabels[m])}
                  </option>
                ))}
              </Field.Select>
            </Field.Root>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.tlAmount)}</Field.Label>
              <Field.Input inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} />
            </Field.Root>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.tlPayer)}</Field.Label>
              <Field.Input value={payer} onChange={(event) => setPayer(event.target.value)} autoComplete="off" />
            </Field.Root>
            {method === 'cash' ? null : (
              <Field.Root required>
                <Field.Label>{intl.formatMessage(messages.tlReference)}</Field.Label>
                <Field.Input value={reference} onChange={(event) => setReference(event.target.value)} autoComplete="off" />
                <Field.HelperText>{intl.formatMessage(messages.tlReferenceHelp)}</Field.HelperText>
              </Field.Root>
            )}
            <button type="submit" className={button} disabled={!canTake || take.isPending}>
              {intl.formatMessage(messages.tlRecord)}
            </button>
            {take.isError ? (
              <p role="alert" className={css({ color: 'danger.text' })}>
                {intl.formatMessage(messages.tlTakeFailed)} {reasonOf(take.error)}
              </p>
            ) : null}
          </form>

          <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(messages.tlReceipts)}</h2>
          <button type="button" className={button} onClick={() => void refresh()}>
            {intl.formatMessage(messages.tlRefresh)}
          </button>
          <ul className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
            {(current.data?.payments ?? []).map((p) => (
              <li key={p.id}>
                {p.receipt.number} · {intl.formatMessage(methodLabels[p.method])} · {p.amount} · <strong>{intl.formatMessage(receiptLabels[p.receipt.status])}</strong>
              </li>
            ))}
          </ul>

          <form
            aria-labelledby="close-shift"
            className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
            onSubmit={(event) => {
              event.preventDefault()
              if (counted.trim() !== '') close.mutate(shift.id)
            }}
          >
            <h2 id="close-shift" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
              {intl.formatMessage(messages.tlClose)}
            </h2>
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.tlCounted)}</Field.Label>
              <Field.Input inputMode="decimal" value={counted} onChange={(event) => setCounted(event.target.value)} />
            </Field.Root>
            <button type="submit" className={button} disabled={counted.trim() === '' || close.isPending}>
              {intl.formatMessage(messages.tlCloseSubmit)}
            </button>
          </form>
        </>
      )}
      {close.data === undefined ? null : (
        <p role="status">
          {intl.formatMessage(close.data.status === 'settled' ? messages.tlSettled : messages.tlAwaiting)} {close.data.variance ?? ''} · {close.data.id}
        </p>
      )}
    </section>
  )
}
