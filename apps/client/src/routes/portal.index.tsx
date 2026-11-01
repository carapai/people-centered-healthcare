import { createFileRoute } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { PortalPage } from '../features/portal/PortalPage'
import { messages } from '../i18n/messages'
import { usePlatform } from '../platform'

export const Route = createFileRoute('/portal/')({
  component: Portal,
})

function Portal() {
  const intl = useIntl()
  const platform = usePlatform()
  return platform.portal === undefined ? <p role="status">{intl.formatMessage(messages.portalUnavailable)}</p> : <PortalPage portal={platform.portal} />
}
