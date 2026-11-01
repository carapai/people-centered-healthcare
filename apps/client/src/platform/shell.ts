// The app shell an edge serves (lan-security.md § App delivery to browsers): its service worker
// keeps the shell loadable across edge restarts, and the version check reloads a shell that a
// newer edge no longer serves (client-application.md § Serving and versions).

/** The edge's service worker; it caches the shell only, never an API response (ADR 0037). */
export function registerShellWorker(host: { readonly serviceWorker?: ServiceWorkerContainer } = navigator) {
  // Browsers expose service workers only in secure contexts (the edge's HTTPS, or localhost).
  if (host.serviceWorker === undefined) return
  host.serviceWorker.register('/sw.js').catch((error: unknown) => {
    console.warn('the app shell service worker did not register', error)
  })
}

export class VersionMismatch extends Error {
  constructor(readonly running: string, readonly served: string) {
    super(`this app is build ${running}, but its edge serves build ${served}`)
    this.name = 'VersionMismatch'
  }
}

interface Reloader {
  /** Whether this page was itself a reload, so a mismatch that survives one is not retried. */
  reloaded(): boolean
  reload(): void
}

const pageReloader: Reloader = {
  reloaded: () =>
    performance.getEntriesByType('navigation').some((entry) => (entry as PerformanceNavigationTiming).type === 'reload'),
  reload: () => window.location.reload(),
}

/**
 * Compares this bundle's build with the one the serving node advertises. A cached shell from
 * before an edge update reloads once to fetch the new one; returns false while reloading.
 */
export function checkVersion(served: string | undefined, running: string, reloader: Reloader = pageReloader): boolean {
  if (served === undefined || served === running) return true
  if (reloader.reloaded()) throw new VersionMismatch(running, served)
  reloader.reload()
  return false
}
