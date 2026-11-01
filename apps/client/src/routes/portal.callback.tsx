import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../i18n/messages'
import { usePlatform } from '../platform'

export const Route = createFileRoute('/portal/callback')({
  component: PortalCallback,
})

/** Where the authorization server returns a patient with a code: redeem it, then go on. */
function PortalCallback() {
  const intl = useIntl()
  const platform = usePlatform()
  const navigate = useNavigate()
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    const signIn = platform.portal?.signIn
    if (signIn === undefined) {
      setFailed(true)
      return
    }
    signIn
      .complete(window.location.href)
      .then((returnTo) => navigate({ to: returnTo, replace: true }))
      .catch(() => setFailed(true))
  }, [platform, navigate])
  return <p role="status">{intl.formatMessage(failed ? messages.signInFailed : messages.signInCompleting)}</p>
}
