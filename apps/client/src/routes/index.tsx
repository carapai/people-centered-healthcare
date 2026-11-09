import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { accessMessages, consultationMessages, edgeStatusMessages, enrollmentMessages, formMessages, inboxMessages, labMessages, messages, pharmacyMessages, receptionMessages, triageMessages, worklistMessages } from '../i18n/messages'
import { SetPin } from '../features/lock/LockGuard'
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
      {/* The edge's own status needs no sign-in yet: the edge serves it to the node itself only. */}
      {platform.capabilities.edgeStatus ? <Link to="/edge/status">{intl.formatMessage(edgeStatusMessages.esTitle)}</Link> : null}
      {/* On an edge, each device must be enrolled before it may use the API (ADR 0009). */}
      {platform.device === undefined ? null : <Link to="/enroll">{intl.formatMessage(enrollmentMessages.enTitle)}</Link>}
      {platform.signIn === undefined ? null : signedIn ? (
        <>
          <p role="status">{intl.formatMessage(messages.signedIn)}</p>
          <nav aria-label={intl.formatMessage(messages.appName)}>
            <ul className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
              <li>
                <Link to="/registration">{intl.formatMessage(messages.regTitle)}</Link>
              </li>
              <li>
                <Link to="/registry/reviews">{intl.formatMessage(messages.rrTitle)}</Link>
              </li>
              <li>
                <Link to="/break-glass/alerts">{intl.formatMessage(messages.bgAlertsTitle)}</Link>
              </li>
              <li>
                <Link to="/break-glass/reviews">{intl.formatMessage(messages.bgReviewsTitle)}</Link>
              </li>
              <li>
                <Link to="/stock">{intl.formatMessage(messages.stTitle)}</Link>
              </li>
              <li>
                <Link to="/till">{intl.formatMessage(messages.tlTitle)}</Link>
              </li>
              <li>
                <Link to="/worklists">{intl.formatMessage(worklistMessages.wlTitle)}</Link>
              </li>
              <li>
                <Link to="/forms">{intl.formatMessage(formMessages.fmTitle)}</Link>
              </li>
              <li>
                <Link to="/reception">{intl.formatMessage(receptionMessages.rcTitle)}</Link>
              </li>
              <li>
                <Link to="/reception/queues">{intl.formatMessage(receptionMessages.rcQueuesTitle)}</Link>
              </li>
              <li>
                <Link to="/triage">{intl.formatMessage(triageMessages.trTitle)}</Link>
              </li>
              <li>
                <Link to="/consultation">{intl.formatMessage(consultationMessages.csTitle)}</Link>
              </li>
              <li>
                <Link to="/pharmacy">{intl.formatMessage(pharmacyMessages.phTitle)}</Link>
              </li>
              <li>
                <Link to="/laboratory">{intl.formatMessage(labMessages.lbTitle)}</Link>
              </li>
              <li>
                <Link to="/results">{intl.formatMessage(labMessages.rsTitle)}</Link>
              </li>
              <li>
                <Link to="/inbox">{intl.formatMessage(inboxMessages.ibTitle)}</Link>
              </li>
              {/* Offline administration lives on the edge (offline-auth.md); it answers non-admins 403. */}
              {platform.device === undefined ? null : (
                <li>
                  <Link to="/access">{intl.formatMessage(accessMessages.acTitle)}</Link>
                </li>
              )}
            </ul>
          </nav>
          <SetPin />
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
