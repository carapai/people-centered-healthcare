import { defineMessages } from 'react-intl'

// The app's strings, as ICU messages (client-application.md § Design system). Ids are
// `app.<feature>.<name>`; the design system's are `ds.*`.
export const messages = defineMessages({
  appName: { id: 'app.shell.name', defaultMessage: 'Health Platform', description: 'Product name on the home screen' },
  modeCentral: { id: 'app.status.mode.central', defaultMessage: 'Central', description: 'Status area: the client talks to the central server' },
  modeEdge: { id: 'app.status.mode.edge', defaultMessage: 'Facility edge', description: 'Status area: the client talks to its facility edge server' },
  modeTauri: { id: 'app.status.mode.tauri', defaultMessage: 'Local runtime', description: 'Status area: the desktop app runs its own local runtime' },
  syncPending: { id: 'app.status.syncPending', defaultMessage: 'Sync status pending', description: 'Status area: sync detail is not available yet' },
  signInTitle: { id: 'app.signIn.title', defaultMessage: 'Sign in', description: 'Sign-in page heading' },
  signInUsername: { id: 'app.signIn.username', defaultMessage: 'Username', description: 'Sign-in page: username field label' },
  signInPassword: { id: 'app.signIn.password', defaultMessage: 'Password', description: 'Sign-in page: password field label' },
  signInSubmit: { id: 'app.signIn.submit', defaultMessage: 'Sign in', description: 'Sign-in page: submit button' },
  signInRefused: {
    id: 'app.signIn.refused',
    defaultMessage: 'The username or password is incorrect, or the account is locked.',
    description: 'Sign-in page: one message for every refusal, so it does not reveal whether an account exists',
  },
  signInFacility: { id: 'app.signIn.facility', defaultMessage: 'Facility code', description: 'Sign-in page: facility field label' },
  signInFacilityHelp: {
    id: 'app.signIn.facilityHelp',
    defaultMessage: 'Needed only if you work at more than one facility.',
    description: 'Sign-in page: helper text under the facility field',
  },
  signInChooseFacility: {
    id: 'app.signIn.chooseFacility',
    defaultMessage: 'You work at more than one facility. Enter the code of the facility you are signing in for, then sign in again.',
    description: 'Sign-in page: shown when the user holds roles at several facilities and named none',
  },
  signInStart: { id: 'app.signIn.start', defaultMessage: 'Sign in', description: 'Home: button that starts sign-in' },
  signOut: { id: 'app.signIn.signOut', defaultMessage: 'Sign out', description: 'Home: button that signs the user out' },
  signedIn: { id: 'app.signIn.signedIn', defaultMessage: 'Signed in', description: 'Home: shown once the session is confirmed by the server' },
  signInCompleting: { id: 'app.signIn.completing', defaultMessage: 'Signing you in…', description: 'Callback page while the code is redeemed' },
  signInFailed: { id: 'app.signIn.failed', defaultMessage: 'Sign-in could not be completed. Please sign in again.', description: 'Callback page when sign-in fails' },
})
