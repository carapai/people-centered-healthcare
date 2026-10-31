import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../i18n/messages'
import { usePlatform } from '../platform'

export const Route = createFileRoute('/callback')({
  component: Callback,
})

/** Where the authorization server returns the browser with a code: redeem it, then move on. */
function Callback() {
  const intl = useIntl()
  const platform = usePlatform()
  const navigate = useNavigate()
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    const signIn = platform.signIn
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
