import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation } from '@tanstack/react-query'
import { type ReactNode, useEffect, useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'

/** Idle this long, the session locks (offline-auth.md § Tokens and sessions). */
export const IDLE_MS = 15 * 60 * 1000

/** What counts as use of the device. */
const ACTIVITY = ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const

const button = css({
  alignSelf: 'flex-start',
  minHeight: 'control',
  px: 'controlX',
  bg: 'accent.default',
  color: 'accent.fg',
  borderRadius: 'md',
  fontSize: 'control',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})

/**
 * Locks a signed-in session after 15 minutes without use, where the node supports it (an edge),
 * and shows the lock screen in place of the page. The lock forgets the tokens and clears the
 * query cache, so nothing of the user's view remains while locked or after a switch.
 */
export function LockGuard({ children }: { children: ReactNode }) {
  const platform = usePlatform()
  const lock = platform.signIn?.lock
  const [locked, setLocked] = useState(false)

  useEffect(() => {
    if (lock === undefined) return undefined
    return platform.session.onEnd((reason) => {
      if (reason === 'lock') setLocked(true)
    })
  }, [lock, platform.session])

  useEffect(() => {
    if (lock === undefined || locked) return undefined
    let timer: ReturnType<typeof setTimeout> | undefined
    const reset = () => {
      if (timer !== undefined) clearTimeout(timer)
      timer = setTimeout(() => {
        if (platform.session.accessToken() !== undefined) lock()
      }, IDLE_MS)
    }
    for (const event of ACTIVITY) window.addEventListener(event, reset, { passive: true })
    reset()
    return () => {
      if (timer !== undefined) clearTimeout(timer)
      for (const event of ACTIVITY) window.removeEventListener(event, reset)
    }
  }, [lock, locked, platform.session])

  return locked ? <LockScreen onUnlocked={() => setLocked(false)} /> : children
}

function LockScreen({ onUnlocked }: { onUnlocked: () => void }) {
  const intl = useIntl()
  const { signIn } = usePlatform()
  const [username, setUsername] = useState('')
  const [pin, setPin] = useState('')
  const unlock = useMutation({
    mutationFn: () => signIn?.unlock?.(username.trim(), pin) ?? Promise.resolve(),
    onSuccess: onUnlocked,
    onError: () => setPin(''),
  })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: 'md' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.lockTitle)}</h1>
      <p>{intl.formatMessage(messages.lockIntro)}</p>
      <form
        className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (username.trim() !== '' && pin !== '') unlock.mutate()
        }}
      >
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.lockUsername)}</Field.Label>
          <Field.Input autoComplete="username" value={username} onChange={(event) => setUsername(event.target.value)} />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.lockPin)}</Field.Label>
          <Field.Input type="password" inputMode="numeric" autoComplete="off" value={pin} onChange={(event) => setPin(event.target.value)} />
        </Field.Root>
        <button type="submit" className={button} disabled={unlock.isPending}>
          {intl.formatMessage(messages.lockSubmit)}
        </button>
        {unlock.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(messages.lockRefused)}
          </p>
        ) : null}
      </form>
      <button type="button" className={css({ alignSelf: 'flex-start', textDecoration: 'underline', cursor: 'pointer' })} onClick={() => void signIn?.start('/')}>
        {intl.formatMessage(messages.lockPassword)}
      </button>
    </section>
  )
}

/** Sets the signed-in user's PIN on this device, where the node supports it (an edge). */
export function SetPin() {
  const intl = useIntl()
  const { signIn } = usePlatform()
  const [pin, setPin] = useState('')
  const save = useMutation({ mutationFn: () => signIn?.setPin?.(pin) ?? Promise.resolve(), onSettled: () => setPin('') })
  if (signIn?.setPin === undefined) return null
  return (
    <form
      aria-labelledby="set-pin"
      className={css({ display: 'flex', flexDirection: 'column', gap: '3', maxWidth: 'md' })}
      onSubmit={(event) => {
        event.preventDefault()
        if (pin !== '') save.mutate()
      }}
    >
      <h2 id="set-pin" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
        {intl.formatMessage(messages.pinTitle)}
      </h2>
      <Field.Root required>
        <Field.Label>{intl.formatMessage(messages.pinField)}</Field.Label>
        <Field.Input type="password" inputMode="numeric" autoComplete="off" value={pin} onChange={(event) => setPin(event.target.value)} />
      </Field.Root>
      <button type="submit" className={button} disabled={save.isPending}>
        {intl.formatMessage(messages.pinSubmit)}
      </button>
      {save.isSuccess ? <p role="status">{intl.formatMessage(messages.pinSaved)}</p> : null}
      {save.isError ? <p role="alert">{intl.formatMessage(messages.pinFailed)}</p> : null}
    </form>
  )
}
