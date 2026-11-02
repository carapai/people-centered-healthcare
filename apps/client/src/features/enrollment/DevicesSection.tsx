import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { edgeStatusMessages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { EnrollmentError, approve, devices, pendingRequests, revoke } from './api'
import { POLL_MS, button } from './EnrollPage'

const section = css({ display: 'flex', flexDirection: 'column', gap: '3' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '2' })
const quiet = css({
  minHeight: 'control',
  px: 'controlX',
  borderRadius: 'md',
  borderWidth: '1px',
  borderColor: 'border.default',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
})

const reasonOf = (error: unknown) => (error instanceof EnrollmentError ? error.code : String(error))

/**
 * The edge's device registry, on its own screen (lan-security.md § Device enrollment): the
 * requests waiting for approval, a form to approve one by the code its device shows, and the
 * enrolled devices, each of which can be revoked.
 */
export function DevicesSection() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const pending = useQuery({ queryKey: ['edge-devices', 'pending'], queryFn: ({ signal }) => pendingRequests(transport, signal), refetchInterval: POLL_MS })
  const enrolled = useQuery({ queryKey: ['edge-devices', 'enrolled'], queryFn: ({ signal }) => devices(transport, signal) })
  const refresh = () => queryClient.invalidateQueries({ queryKey: ['edge-devices'] })
  const [code, setCode] = useState('')
  const [name, setName] = useState('')
  const [location, setLocation] = useState('')
  const [confirming, setConfirming] = useState<string | null>(null)
  const approval = useMutation({
    mutationFn: () => approve(transport, { code: code.trim(), name: name.trim(), location: location.trim() }),
    onSuccess: () => {
      setCode('')
      setName('')
      setLocation('')
    },
    onSettled: refresh,
  })
  const revocation = useMutation({ mutationFn: (id: string) => revoke(transport, id), onSettled: refresh, onSuccess: () => setConfirming(null) })
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  return (
    <section className={section} aria-labelledby="edge-devices">
      <h2 id="edge-devices" className={subheading}>
        {intl.formatMessage(edgeStatusMessages.esDevices)}
      </h2>

      <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(edgeStatusMessages.esPendingRequests)}</h3>
      {pending.data === undefined || pending.data.length === 0 ? (
        <p>{intl.formatMessage(edgeStatusMessages.esNoPending)}</p>
      ) : (
        <ul className={list}>
          {pending.data.map((r) => (
            <li key={r.id}>{intl.formatMessage(edgeStatusMessages.esRequested, { label: r.label, expires: time(r.expires) })}</li>
          ))}
        </ul>
      )}

      <form
        aria-labelledby="edge-approve"
        className={css({ display: 'flex', flexDirection: 'column', gap: '3', maxWidth: 'md' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (code.trim() !== '' && name.trim() !== '' && location.trim() !== '') approval.mutate()
        }}
      >
        <h3 id="edge-approve" className={css({ fontWeight: 'semibold' })}>
          {intl.formatMessage(edgeStatusMessages.esApprove)}
        </h3>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(edgeStatusMessages.esApproveCode)}</Field.Label>
          <Field.Input inputMode="numeric" autoComplete="off" value={code} onChange={(event) => setCode(event.target.value)} />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(edgeStatusMessages.esDeviceName)}</Field.Label>
          <Field.Input autoComplete="off" value={name} onChange={(event) => setName(event.target.value)} />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(edgeStatusMessages.esDeviceLocation)}</Field.Label>
          <Field.Input autoComplete="off" value={location} onChange={(event) => setLocation(event.target.value)} />
        </Field.Root>
        <button type="submit" className={button} disabled={approval.isPending}>
          {intl.formatMessage(edgeStatusMessages.esApprove)}
        </button>
        {approval.isSuccess ? <p role="status">{intl.formatMessage(edgeStatusMessages.esApproved, { name: approval.data.name })}</p> : null}
        {approval.isError ? <p role="alert">{intl.formatMessage(edgeStatusMessages.esActionFailed, { reason: reasonOf(approval.error) })}</p> : null}
      </form>

      <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(edgeStatusMessages.esEnrolledDevices)}</h3>
      {enrolled.data === undefined || enrolled.data.length === 0 ? (
        <p>{intl.formatMessage(edgeStatusMessages.esNoDevices)}</p>
      ) : (
        <ul className={list}>
          {enrolled.data.map((d) => (
            <li key={d.id} className={css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center' })}>
              <span>
                {intl.formatMessage(edgeStatusMessages.esDeviceLine, { name: d.name, location: d.location, label: d.label })}
                {d.revoked_at === null ? null : ` — ${intl.formatMessage(edgeStatusMessages.esDeviceRevokedAt, { at: time(d.revoked_at) })}`}
              </span>
              {d.status !== 'active' ? null : confirming === d.id ? (
                <>
                  <button type="button" className={button} disabled={revocation.isPending} onClick={() => revocation.mutate(d.id)}>
                    {intl.formatMessage(edgeStatusMessages.esConfirmRevoke, { name: d.name })}
                  </button>
                  <button type="button" className={quiet} onClick={() => setConfirming(null)}>
                    {intl.formatMessage(edgeStatusMessages.esCancel)}
                  </button>
                </>
              ) : (
                <button type="button" className={quiet} onClick={() => setConfirming(d.id)}>
                  {intl.formatMessage(edgeStatusMessages.esRevoke, { name: d.name })}
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      {revocation.isError ? <p role="alert">{intl.formatMessage(edgeStatusMessages.esActionFailed, { reason: reasonOf(revocation.error) })}</p> : null}
    </section>
  )
}
