import { css } from '@health/design-system/css'
import { createFileRoute } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { messages } from '../i18n/messages'

export const Route = createFileRoute('/')({
  component: Home,
})

function Home() {
  const intl = useIntl()
  return <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.appName)}</h1>
}
