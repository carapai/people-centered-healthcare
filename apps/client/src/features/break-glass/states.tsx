import { css } from '@health/design-system/css'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { BreakGlassError } from './api'

/** What a list page shows while loading or after a failure: 403 is the user's role, not an error. */
export function ListState({ loading, error }: { loading: boolean; error: unknown }) {
  const intl = useIntl()
  if (loading) return <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
  const forbidden = error instanceof BreakGlassError && error.status === 403
  return (
    <p role="alert" className={css({ color: forbidden ? 'fg.muted' : 'danger.text' })}>
      {intl.formatMessage(forbidden ? messages.bgForbidden : messages.bgFailed)}
    </p>
  )
}

export const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
export const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
export const action = css({
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
