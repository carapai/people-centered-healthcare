import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { TillError, approveShift, reconcile } from './api'
import { button } from './TillPage'

const reasonOf = (error: unknown) => (error instanceof TillError ? error.reason : '')

/**
 * A supervisor's payments work (#254): enter a provider statement line, which confirms the
 * pending payment it matches, and approve a closed shift's variance. The server checks the
 * supervise permission.
 */
export function SupervisePage() {
  const intl = useIntl()
  const platform = usePlatform()
  const [reference, setReference] = useState('')
  const [amount, setAmount] = useState('')
  const [shift, setShift] = useState('')
  const statement = useMutation({ mutationFn: () => reconcile(platform.transport, reference.trim(), amount.trim()) })
  const approve = useMutation({ mutationFn: () => approveShift(platform.transport, shift.trim()) })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '2xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.tlSuperviseTitle)}</h1>
      <form
        aria-labelledby="statement"
        className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (reference.trim() !== '' && amount.trim() !== '') statement.mutate()
        }}
      >
        <h2 id="statement" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
          {intl.formatMessage(messages.tlStatement)}
        </h2>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.tlReference)}</Field.Label>
          <Field.Input value={reference} onChange={(event) => setReference(event.target.value)} autoComplete="off" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.tlAmount)}</Field.Label>
          <Field.Input inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} />
        </Field.Root>
        <button type="submit" className={button} disabled={statement.isPending}>
          {intl.formatMessage(messages.tlReconcile)}
        </button>
        {statement.data === undefined ? null : (
          <p role="status">
            {intl.formatMessage(messages.tlReconciled)} {statement.data.confirmed.length} · {intl.formatMessage(messages.tlExceptions)}{' '}
            {statement.data.mismatched.length + statement.data.unknown.length}
          </p>
        )}
        {statement.isError ? <p role="alert">{reasonOf(statement.error)}</p> : null}
      </form>
      <form
        aria-labelledby="approve"
        className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (shift.trim() !== '') approve.mutate()
        }}
      >
        <h2 id="approve" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
          {intl.formatMessage(messages.tlApprove)}
        </h2>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.tlShift)}</Field.Label>
          <Field.Input value={shift} onChange={(event) => setShift(event.target.value)} autoComplete="off" />
        </Field.Root>
        <button type="submit" className={button} disabled={approve.isPending}>
          {intl.formatMessage(messages.tlApproveSubmit)}
        </button>
        {approve.data === undefined ? null : <p role="status">{intl.formatMessage(messages.tlSettled)} {approve.data.variance ?? ''}</p>}
        {approve.isError ? <p role="alert">{reasonOf(approve.error)}</p> : null}
      </form>
    </section>
  )
}
