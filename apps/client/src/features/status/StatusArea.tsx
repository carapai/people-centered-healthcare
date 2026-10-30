import { css } from '@health/design-system/css'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'

const labels = {
  central: messages.modeCentral,
  edge: messages.modeEdge,
  tauri: messages.modeTauri,
} as const

/** The persistent status area (§101). Upstream reachability and sync detail arrive with their ports. */
export function StatusArea() {
  const platform = usePlatform()
  const intl = useIntl()
  return (
    <header
      role="status"
      className={css({ display: 'flex', gap: '4', px: '6', py: '2', bg: 'bg.subtle', color: 'fg.default', fontSize: 'sm' })}
    >
      <span>{intl.formatMessage(labels[platform.mode])}</span>
      {platform.capabilities.syncStatus ? <span>{intl.formatMessage(messages.syncPending)}</span> : null}
    </header>
  )
}
