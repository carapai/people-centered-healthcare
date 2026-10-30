import { defineMessages } from 'react-intl'

// The app's strings, as ICU messages (client-application.md § Design system). Ids are
// `app.<feature>.<name>`; the design system's are `ds.*`.
export const messages = defineMessages({
  appName: { id: 'app.shell.name', defaultMessage: 'Health Platform', description: 'Product name on the home screen' },
  modeCentral: { id: 'app.status.mode.central', defaultMessage: 'Central', description: 'Status area: the client talks to the central server' },
  modeEdge: { id: 'app.status.mode.edge', defaultMessage: 'Facility edge', description: 'Status area: the client talks to its facility edge server' },
  modeTauri: { id: 'app.status.mode.tauri', defaultMessage: 'Local runtime', description: 'Status area: the desktop app runs its own local runtime' },
  syncPending: { id: 'app.status.syncPending', defaultMessage: 'Sync status pending', description: 'Status area: sync detail is not available yet' },
})
