import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { createFileRoute } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { messages } from '../i18n/messages'
import { usePlatform } from '../platform'

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
 * through application code, and comes back here with `error=refused` on any refusal.
 */
function SignInPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const search = Route.useSearch()
  const action = platform.signIn?.authorizeEndpoint ?? '/auth/authorize'
  return (
    <section className={css({ maxWidth: 'sm', mx: 'auto', display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.signInTitle)}</h1>
      {search['error'] === 'refused' ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.signInRefused)}
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
