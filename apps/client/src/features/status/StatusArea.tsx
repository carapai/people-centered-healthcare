import { css } from '../../../styled-system/css'
import { usePlatform } from '../../platform'

const labels = {
  central: 'Central',
  edge: 'Facility edge',
  tauri: 'Local runtime',
} as const

/** The persistent status area (§101). Upstream reachability and sync detail arrive with their ports. */
export function StatusArea() {
  const platform = usePlatform()
  return (
    <header
      role="status"
      className={css({ display: 'flex', gap: '4', px: '6', py: '2', bg: 'gray.100', fontSize: 'sm' })}
    >
      <span>{labels[platform.mode]}</span>
      {platform.capabilities.syncStatus ? <span>Sync status pending</span> : null}
    </header>
  )
}
