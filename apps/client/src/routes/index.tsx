import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { messages } from '../i18n/messages'
import { usePlatform } from '../platform'

export const Route = createFileRoute('/')({
  component: Home,
})

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
})

function Home() {
  const intl = useIntl()
  const platform = usePlatform()
  // `/session` is an authorized route: it answers only with a valid access token.
  const session = useQuery({
    queryKey: ['session'],
    queryFn: ({ signal }) => platform.transport.request({ method: 'GET', path: '/session', signal }),
    enabled: platform.session.accessToken() !== undefined,
  })
  const signedIn = session.data?.status === 200
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.appName)}</h1>
      {platform.signIn === undefined ? null : signedIn ? (
        <>
          <p role="status">{intl.formatMessage(messages.signedIn)}</p>
          <button type="button" className={button} onClick={() => void platform.signIn?.signOut().then(() => session.refetch())}>
            {intl.formatMessage(messages.signOut)}
          </button>
        </>
      ) : (
        <button type="button" className={button} onClick={() => void platform.signIn?.start('/')}>
          {intl.formatMessage(messages.signInStart)}
        </button>
      )}
    </section>
  )
}
