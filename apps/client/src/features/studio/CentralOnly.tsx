import type { ReactNode } from 'react'
import { useIntl } from 'react-intl'

import { studioMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'

/** The Studio's screens, only where the platform offers the Studio: at central, never on an
 * edge or a desktop (studio.md § Scope). Elsewhere there is no `/api/studio` to call. */
export function CentralOnly({ children }: { children: ReactNode }) {
  const intl = useIntl()
  const platform = usePlatform()
  return platform.capabilities.studio ? children : <p role="status">{intl.formatMessage(m.sdUnavailable)}</p>
}
