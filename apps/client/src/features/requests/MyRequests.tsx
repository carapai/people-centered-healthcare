import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { requestMessages as m } from '../../i18n/messages'
import { newResourceId } from '../../platform/ids'
import type { Transport } from '../../platform/types'
import { button, control, field } from '../reception/styles'
import { MY_REQUESTS, RequestFailed, ask, myPrescriptionsQuery, myRequestsQuery, prescriptionReference, withdraw, type Ask, type PatientRequest } from './api'

type Kind = Ask['kind']

const failureOf = (error: unknown) => (error instanceof RequestFailed ? error.reason : '')

/** One of the patient's requests, withdrawable while it waits for staff. */
function Item({ request, transport }: { request: PatientRequest; transport: Transport }) {
  const intl = useIntl()
  const queryClient = useQueryClient()
  const back = useMutation({
    mutationFn: () => withdraw(transport, request.reference),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: MY_REQUESTS }),
  })
  return (
    <li className={css({ display: 'flex', flexDirection: 'column', gap: '1', borderBottom: '1px solid', borderColor: 'border', pb: '2' })}>
      <span className={css({ fontWeight: 'semibold' })}>{request.title}</span>
      <span>
        {intl.formatMessage(m.rqKind, { kind: request.kind })} · {intl.formatMessage(m.rqState, { state: request.state })}
      </span>
      {request.reason === null ? null : <span>{intl.formatMessage(m.rqReasonGiven, { reason: request.reason })}</span>}
      {request.state === 'pending' ? (
        <button type="button" className={button} disabled={back.isPending} onClick={() => back.mutate()}>
          {intl.formatMessage(m.rqWithdraw, { title: request.title })}
        </button>
      ) : null}
      {back.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.rqFailed, { reason: failureOf(back.error) })}
        </p>
      ) : null}
    </li>
  )
}

/**
 * A patient's requests from the portal (#114): a refill of one of their prescriptions, a test or
 * other care, or an appointment. Each is a proposal staff accept or decline; nothing here orders
 * or books anything.
 */
export function MyRequests({ transport, patient, full }: { transport: Transport; patient: string; full: boolean }) {
  const intl = useIntl()
  const queryClient = useQueryClient()
  const ids = { kind: useId(), prior: useId(), what: useId(), facility: useId(), hint: useId(), preferred: useId(), note: useId() }
  const requests = useQuery(myRequestsQuery(transport))
  const prescriptions = useQuery({ ...myPrescriptionsQuery(transport), enabled: full })
  const [kind, setKind] = useState<Kind>('care')
  const [form, setForm] = useState({ prior: '', what: '', facility: '', preferred: '', note: '' })
  const [sent, setSent] = useState(false)
  const set = (key: keyof typeof form) => (event: { target: { value: string } }) => setForm({ ...form, [key]: event.target.value })
  const asked = (): Ask | undefined => {
    if (kind === 'refill') return form.prior === '' ? undefined : { kind, prior: prescriptionReference(form.prior) }
    if (form.what.trim() === '' || form.facility.trim() === '') return undefined
    if (kind === 'care') return { kind, facility: form.facility.trim(), what: form.what.trim() }
    return { kind, facility: form.facility.trim(), reason: form.what.trim(), ...(form.preferred === '' ? {} : { preferred: form.preferred }) }
  }
  const send = useMutation({
    mutationFn: (request: Ask) => ask(transport, newResourceId(), patient, request, form.note),
    onSuccess: () => {
      setSent(true)
      setForm({ prior: '', what: '', facility: form.facility, preferred: '', note: '' })
      void queryClient.invalidateQueries({ queryKey: MY_REQUESTS })
    },
  })
  const kinds: Kind[] = full ? ['care', 'appointment', 'refill'] : ['care', 'appointment']
  const request = asked()
  return (
    <section aria-labelledby="requests" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <h2 id="requests" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
        {intl.formatMessage(m.rqTitle)}
      </h2>
      {requests.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.rqLoadFailed)}
        </p>
      ) : null}
      {requests.data?.length === 0 ? <p>{intl.formatMessage(m.rqNone)}</p> : null}
      <ul aria-label={intl.formatMessage(m.rqTitle)} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {(requests.data ?? []).map((r) => (
          <Item key={r.reference} request={r} transport={transport} />
        ))}
      </ul>
      <div>
        <button type="button" className={button} disabled={requests.isFetching} onClick={() => void requests.refetch()}>
          {intl.formatMessage(m.rqRefresh)}
        </button>
      </div>
      <form
        className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (request !== undefined) send.mutate(request)
        }}
      >
        <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.rqAsk)}</h3>
        <div className={field}>
          <label htmlFor={ids.kind}>{intl.formatMessage(m.rqWhatKind)}</label>
          <select
            id={ids.kind}
            value={kind}
            onChange={(e) => {
              setKind(e.target.value as Kind)
              setSent(false)
            }}
            className={control}
          >
            {kinds.map((k) => (
              <option key={k} value={k}>
                {intl.formatMessage(m.rqKind, { kind: k })}
              </option>
            ))}
          </select>
        </div>
        {kind === 'refill' ? (
          prescriptions.data?.length === 0 ? (
            <p>{intl.formatMessage(m.rqNoPrescriptions)}</p>
          ) : (
            <div className={field}>
              <label htmlFor={ids.prior}>{intl.formatMessage(m.rqPrescription)}</label>
              <select id={ids.prior} value={form.prior} onChange={set('prior')} className={control}>
                <option value="" />
                {(prescriptions.data ?? []).map((p) => (
                  <option key={p.id} value={p.id}>
                    {`${p.medicine.text}, ${p.dosage}`}
                  </option>
                ))}
              </select>
            </div>
          )
        ) : (
          <>
            <div className={field}>
              <label htmlFor={ids.what}>{intl.formatMessage(kind === 'care' ? m.rqWhat : m.rqFor)}</label>
              <input id={ids.what} value={form.what} onChange={set('what')} className={control} />
            </div>
            <div className={field}>
              <label htmlFor={ids.facility}>{intl.formatMessage(m.rqFacility)}</label>
              <input id={ids.facility} aria-describedby={ids.hint} value={form.facility} onChange={set('facility')} className={control} />
              <span id={ids.hint}>{intl.formatMessage(m.rqFacilityHint)}</span>
            </div>
            {kind === 'appointment' ? (
              <div className={field}>
                <label htmlFor={ids.preferred}>{intl.formatMessage(m.rqPreferred)}</label>
                <input id={ids.preferred} type="date" value={form.preferred} onChange={set('preferred')} className={control} />
              </div>
            ) : null}
          </>
        )}
        <div className={field}>
          <label htmlFor={ids.note}>{intl.formatMessage(m.rqNote)}</label>
          <input id={ids.note} value={form.note} onChange={set('note')} className={control} />
        </div>
        <div>
          <button type="submit" className={button} disabled={request === undefined || send.isPending}>
            {intl.formatMessage(m.rqSend)}
          </button>
        </div>
        {sent ? <p role="status">{intl.formatMessage(m.rqSent)}</p> : null}
        {send.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(m.rqFailed, { reason: failureOf(send.error) })}
          </p>
        ) : null}
      </form>
    </section>
  )
}
