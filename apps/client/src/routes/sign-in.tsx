import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { createFileRoute } from '@tanstack/react-router'
import { useRef, useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../i18n/messages'
import { usePlatform } from '../platform'
import { SignInError } from '../platform/sign-in'

/** The authorization request's parameters, passed through untouched to `/authorize`. */
const PASSED = ['response_type', 'client_id', 'redirect_uri', 'scope', 'state', 'nonce', 'code_challenge', 'code_challenge_method'] as const

export const Route = createFileRoute('/sign-in')({
  validateSearch: (search: Record<string, unknown>): Record<string, string> =>
    Object.fromEntries(Object.entries(search).filter((entry): entry is [string, string] => typeof entry[1] === 'string')),
  component: SignInPage,
})

/**
 * The sign-in page the authorization server sends the browser to (offline-auth.md). It posts
 * the credentials straight to `/authorize` as an ordinary form, so the password never passes
 * through application code, and comes back here with `error=refused` on any refusal, or with
 * `error=facility` when the user works at several facilities and named none (ADR 0056), or
 * with `error=change_password` when the account's password is a temporary one an admin set.
 */
function SignInPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const search = Route.useSearch()
  const action = platform.signIn?.authorizeEndpoint ?? '/auth/authorize'
  if (platform.portal !== undefined && search['client_id'] === platform.portal.clientId) {
    return <PhoneSignIn search={search} action={platform.portal.signIn.authorizeEndpoint} />
  }
  if (search['error'] === 'change_password' && platform.signIn !== undefined) {
    return <ChangePassword search={search} action={action} change={(username, password, newPassword) => platform.signIn!.changePassword(username, password, newPassword)} />
  }
  return (
    <section className={css({ maxWidth: 'sm', mx: 'auto', display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.signInTitle)}</h1>
      {search['error'] === 'refused' ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.signInRefused)}
        </p>
      ) : null}
      {search['error'] === 'facility' ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.signInChooseFacility)}
        </p>
      ) : null}
      <form method="post" action={action} className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        {PASSED.map((name) => (search[name] === undefined ? null : <input key={name} type="hidden" name={name} value={search[name]} />))}
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.signInUsername)}</Field.Label>
          <Field.Input name="username" autoComplete="username" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.signInPassword)}</Field.Label>
          <Field.Input name="password" type="password" autoComplete="current-password" />
        </Field.Root>
        <Field.Root required={search['error'] === 'facility'}>
          <Field.Label>{intl.formatMessage(messages.signInFacility)}</Field.Label>
          <Field.Input name="facility" autoComplete="off" />
          <Field.HelperText>{intl.formatMessage(messages.signInFacilityHelp)}</Field.HelperText>
        </Field.Root>
        <button
          type="submit"
          className={css({ minHeight: 'control', px: 'controlX', bg: 'accent.default', color: 'accent.fg', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' } })}
        >
          {intl.formatMessage(messages.signInSubmit)}
        </button>
      </form>
    </section>
  )
}

const CHANGE_ERRORS = {
  mismatch: messages.signInMismatch,
  refused: messages.signInRefused,
  rejected: messages.signInRejected,
  failed: messages.signInChangeFailed,
} as const

/**
 * The first sign-in of an account an admin created offline (offline-auth.md § Offline user and
 * role administration): the temporary password, then the user's own, set at `/password`; then
 * the sign-in carries on to `/authorize` with the new password, as the ordinary form does.
 */
function ChangePassword({
  search,
  action,
  change,
}: {
  search: Record<string, string>
  action: string
  change: (username: string, password: string, newPassword: string) => Promise<void>
}) {
  const intl = useIntl()
  const form = useRef<HTMLFormElement>(null)
  const chosen = useRef<HTMLInputElement>(null)
  const [error, setError] = useState<keyof typeof CHANGE_ERRORS | null>(null)
  const [busy, setBusy] = useState(false)
  return (
    <section className={css({ maxWidth: 'sm', mx: 'auto', display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.signInChangeTitle)}</h1>
      <p>{intl.formatMessage(messages.signInChangeIntro)}</p>
      {error === null ? null : (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(CHANGE_ERRORS[error])}
        </p>
      )}
      <form
        ref={form}
        method="post"
        action={action}
        className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}
        onSubmit={(event) => {
          event.preventDefault()
          const data = new FormData(event.currentTarget)
          const field = (name: string) => String(data.get(name) ?? '')
          if (field('new_password') !== field('confirm_password')) {
            setError('mismatch')
            return
          }
          setBusy(true)
          change(field('username'), field('current_password'), field('new_password')).then(
            () => {
              // Signed in with the new password, through the ordinary form post.
              if (chosen.current !== null) chosen.current.value = field('new_password')
              form.current?.submit()
            },
            (failure: unknown) => {
              setBusy(false)
              setError(failure instanceof SignInError && (failure.reason === 'refused' || failure.reason === 'rejected') ? failure.reason : 'failed')
            },
          )
        }}
      >
        {PASSED.map((name) => (search[name] === undefined ? null : <input key={name} type="hidden" name={name} value={search[name]} />))}
        <input ref={chosen} type="hidden" name="password" />
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.signInUsername)}</Field.Label>
          <Field.Input name="username" autoComplete="username" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.signInCurrentPassword)}</Field.Label>
          <Field.Input name="current_password" type="password" autoComplete="current-password" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.signInNewPassword)}</Field.Label>
          <Field.Input name="new_password" type="password" autoComplete="new-password" />
          <Field.HelperText>{intl.formatMessage(messages.signInNewPasswordHelp)}</Field.HelperText>
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.signInConfirmPassword)}</Field.Label>
          <Field.Input name="confirm_password" type="password" autoComplete="new-password" />
        </Field.Root>
        <button type="submit" className={submitButton} disabled={busy}>
          {intl.formatMessage(messages.signInChangeSubmit)}
        </button>
      </form>
    </section>
  )
}

const submitButton = css({
  minHeight: 'control',
  px: 'controlX',
  bg: 'accent.default',
  color: 'accent.fg',
  borderRadius: 'md',
  fontSize: 'control',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
})

const PHONE_ERRORS = {
  phone: messages.signInPhoneInvalid,
  wait: messages.signInWait,
  refused: messages.signInCodeRefused,
  expired: messages.signInCodeExpired,
} as const

/**
 * A patient's sign-in (#111; ADR 0029): their phone number, posted to `/otp`, which sends a
 * code and comes back here with `otp=sent`; then the code, posted to `/authorize`. There is no
 * patient password.
 */
function PhoneSignIn({ search, action }: { search: Record<string, string>; action: string }) {
  const intl = useIntl()
  const otp = action.replace(/\/authorize$/, '/otp')
  const sent = search['otp'] === 'sent'
  const error = search['error'] as keyof typeof PHONE_ERRORS | undefined
  const hidden = PASSED.map((name) => (search[name] === undefined ? null : <input key={name} type="hidden" name={name} value={search[name]} />))
  return (
    <section className={css({ maxWidth: 'sm', mx: 'auto', display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.signInTitle)}</h1>
      {error !== undefined && error in PHONE_ERRORS ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(PHONE_ERRORS[error])}
        </p>
      ) : null}
      {sent ? (
        <>
          <p role="status">{intl.formatMessage(messages.signInCodeSent)}</p>
          <form method="post" action={action} className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
            {hidden}
            <input type="hidden" name="phone" value={search['phone'] ?? ''} />
            <Field.Root required>
              <Field.Label>{intl.formatMessage(messages.signInCode)}</Field.Label>
              <Field.Input name="code" inputMode="numeric" autoComplete="one-time-code" />
            </Field.Root>
            <button type="submit" className={submitButton}>
              {intl.formatMessage(messages.signInSubmit)}
            </button>
          </form>
          <form method="post" action={otp}>
            {hidden}
            <input type="hidden" name="phone" value={search['phone'] ?? ''} />
            <button type="submit" className={submitButton}>
              {intl.formatMessage(messages.signInNewCode)}
            </button>
          </form>
        </>
      ) : (
        <form method="post" action={otp} className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
          {hidden}
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.signInPhone)}</Field.Label>
            <Field.Input name="phone" type="tel" autoComplete="tel" />
            <Field.HelperText>{intl.formatMessage(messages.signInPhoneHelp)}</Field.HelperText>
          </Field.Root>
          <button type="submit" className={submitButton}>
            {intl.formatMessage(messages.signInSendCode)}
          </button>
        </form>
      )}
    </section>
  )
}
