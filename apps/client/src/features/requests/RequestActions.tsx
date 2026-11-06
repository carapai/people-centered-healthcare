import { css } from '@health/design-system/css'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { requestMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button, control, field } from '../reception/styles'
import { RequestFailed, accept, decline, type Acceptance } from './api'

/** A `datetime-local` value as an instant, in the browser's time zone. */
const instant = (local: string) => (local === '' ? undefined : new Date(local).toISOString())

/**
 * Staff's step on a patient's request (#114, §60): accept it, which writes its order or booking
 * (a refill through the medication safety checks), or decline it with a reason. Only this turns a
 * request into an order. `onDone` hears the outcome; the record then reloads, no longer pending.
 */
export function RequestActions({ reference, appointment, onDone }: { reference: string; appointment: boolean; onDone: (message: string) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { start: useId(), end: useId(), reason: useId() }
  const [time, setTime] = useState({ start: '', end: '' })
  const [reason, setReason] = useState('')
  const [type, id] = reference.split('/')
  const finished = (message: string) => {
    onDone(message)
    void queryClient.invalidateQueries({ queryKey: ['fhir', type, id] })
    void queryClient.invalidateQueries({ queryKey: ['inbox'] })
  }
  const accepting = useMutation({
    mutationFn: (acceptance: Acceptance) => accept(platform.transport, reference, acceptance),
    onSuccess: () => finished(intl.formatMessage(m.rqAccepted)),
  })
  const declining = useMutation({
    mutationFn: () => decline(platform.transport, reference, reason.trim()),
    onSuccess: () => finished(intl.formatMessage(m.rqDeclined)),
  })
  const booking = { start: instant(time.start), end: instant(time.end) }
  const acceptance: Acceptance = appointment ? { ...(booking.start === undefined ? {} : { start: booking.start }), ...(booking.end === undefined ? {} : { end: booking.end }) } : {}
  const failed = accepting.error ?? declining.error
  const warnings = accepting.error instanceof RequestFailed ? accepting.error.warnings : []
  const busy = accepting.isPending || declining.isPending
  return (
    <section aria-labelledby="request-actions" className={css({ display: 'flex', flexDirection: 'column', gap: '3', borderTop: '1px solid', borderColor: 'border', pt: '3' })}>
      <h2 id="request-actions" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
        {intl.formatMessage(m.rqStaffTitle)}
      </h2>
      {appointment ? (
        <>
          <div className={field}>
            <label htmlFor={ids.start}>{intl.formatMessage(m.rqStart)}</label>
            <input id={ids.start} type="datetime-local" value={time.start} onChange={(e) => setTime({ ...time, start: e.target.value })} className={control} />
          </div>
          <div className={field}>
            <label htmlFor={ids.end}>{intl.formatMessage(m.rqEnd)}</label>
            <input id={ids.end} type="datetime-local" value={time.end} onChange={(e) => setTime({ ...time, end: e.target.value })} className={control} />
          </div>
        </>
      ) : null}
      <div>
        <button
          type="button"
          className={button}
          disabled={busy || (appointment && (booking.start === undefined || booking.end === undefined))}
          onClick={() => accepting.mutate(acceptance)}
        >
          {intl.formatMessage(m.rqAccept)}
        </button>
      </div>
      {warnings.length === 0 ? null : (
        <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          <p>{intl.formatMessage(m.rqWarnings)}</p>
          <ul>
            {warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
          <div>
            <button type="button" className={button} disabled={busy} onClick={() => accepting.mutate({ ...acceptance, acknowledge: true })}>
              {intl.formatMessage(m.rqAcceptAnyway)}
            </button>
          </div>
        </div>
      )}
      <div className={field}>
        <label htmlFor={ids.reason}>{intl.formatMessage(m.rqDeclineReason)}</label>
        <input id={ids.reason} value={reason} onChange={(e) => setReason(e.target.value)} className={control} />
      </div>
      <div>
        <button type="button" className={button} disabled={busy || reason.trim() === ''} onClick={() => declining.mutate()}>
          {intl.formatMessage(m.rqDecline)}
        </button>
      </div>
      {failed !== null && warnings.length === 0 ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.rqFailed, { reason: failed instanceof RequestFailed ? failed.reason : '' })}
        </p>
      ) : null}
    </section>
  )
}
