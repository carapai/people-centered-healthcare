import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { enrollmentMessages as messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { EnrollmentError, deviceLabel, enrollmentStatus, requestEnrollment, thisDevice } from './api'

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

const code = css({ fontFamily: 'mono', fontSize: '4xl', fontWeight: 'semibold', letterSpacing: 'widest' })

/** How often a waiting device asks whether it was approved. */
export const POLL_MS = 3_000

const reasonOf = (error: unknown) => (error instanceof EnrollmentError ? error.code : String(error))

/**
 * A device asks its facility edge to enroll it (lan-security.md § Device enrollment; ADR 0009):
 * it makes a key it cannot export, sends a request proved with it, and shows the pairing code an
 * admin types on the edge to approve it. A revoked device starts over with a new key.
 */
export function EnrollPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const device = platform.device
  // Whether a request was pending, to tell an expired one from none at all.
  const [waited, setWaited] = useState(false)
  // React state, so the status query starts once the device has made its key.
  const [hasKey, setHasKey] = useState(device?.current() !== undefined)
  const status = useQuery({
    queryKey: ['enrollment'],
    queryFn: ({ signal }) => enrollmentStatus(platform.transport, signal),
    enabled: hasKey,
    refetchInterval: (query) => (query.state.data?.status === 'pending' ? POLL_MS : false),
  })
  const enrolled = status.data?.status === 'enrolled'
  // Through the gate: proof that this device's requests are now accepted.
  const confirmed = useQuery({
    queryKey: ['enrollment', 'device'],
    queryFn: ({ signal }) => thisDevice(platform.transport, signal),
    enabled: enrolled,
  })
  const ask = useMutation({
    mutationFn: async () => {
      if (device === undefined) return
      if (device.current() === undefined) await device.create()
      await requestEnrollment(platform.transport, deviceLabel(navigator.userAgent))
    },
    // Only now, so no earlier status read can land after the request and hide it.
    onSettled: (_, error) => {
      setHasKey(device?.current() !== undefined)
      if (error === null) setWaited(true)
      void queryClient.refetchQueries({ queryKey: ['enrollment'] })
    },
  })
  const startOver = useMutation({
    mutationFn: async () => {
      await device?.forget()
      setWaited(false)
      setHasKey(false)
      queryClient.removeQueries({ queryKey: ['enrollment'] })
    },
  })
  const time = (iso: string) => intl.formatTime(new Date(iso), { timeStyle: 'short' })

  let body
  if (device === undefined) {
    body = <p>{intl.formatMessage(messages.enNotEdge)}</p>
  } else if (status.data?.status === 'pending') {
    body = (
      <>
        <p>{intl.formatMessage(messages.enCode)}</p>
        <p className={code} aria-label={intl.formatMessage(messages.enCode)}>
          {status.data.code}
        </p>
        <p>{intl.formatMessage(messages.enShowCode, { expires: time(status.data.expires) })}</p>
        <p role="status">{intl.formatMessage(messages.enWaiting)}</p>
      </>
    )
  } else if (status.data?.status === 'enrolled') {
    const named = confirmed.data ?? status.data.device
    body = <p role="status">{intl.formatMessage(messages.enEnrolled, { name: named.name, location: named.location })}</p>
  } else if (status.data?.status === 'revoked') {
    body = (
      <>
        <p role="alert">{intl.formatMessage(messages.enRevoked)}</p>
        <button type="button" className={button} disabled={startOver.isPending} onClick={() => startOver.mutate()}>
          {intl.formatMessage(messages.enStartOver)}
        </button>
      </>
    )
  } else {
    body = (
      <>
        <p>{intl.formatMessage(messages.enIntro)}</p>
        {waited && status.data?.status === 'none' ? <p role="alert">{intl.formatMessage(messages.enExpired)}</p> : null}
        <button type="button" className={button} disabled={ask.isPending || status.isFetching} onClick={() => ask.mutate()}>
          {intl.formatMessage(messages.enStart)}
        </button>
      </>
    )
  }
  const failure = ask.error ?? status.error ?? confirmed.error
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '2xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.enTitle)}</h1>
      {body}
      {failure === null ? null : (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.enFailed, { reason: reasonOf(failure) })}
        </p>
      )}
    </section>
  )
}
