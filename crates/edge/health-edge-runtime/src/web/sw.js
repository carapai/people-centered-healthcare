// The edge's app-shell service worker (lan-security.md § App delivery to browsers, ADR 0037).
// The edge prepends BUILD and SHELL: its build and the shell's paths (the page, runtime.json and
// the bundle's files). Installing caches exactly those, and nothing is ever added later, so no
// API response is cached; every other request goes straight to the network.
const CACHE = `shell-${BUILD}`
const PAGE = '/'
const CACHED = new Set(SHELL)

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(SHELL))
      .then(() => self.skipWaiting()),
  )
})

// A new build's worker drops the older shells.
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(names.filter((name) => name !== CACHE).map((name) => caches.delete(name))))
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', (event) => {
  const request = event.request
  const url = new URL(request.url)
  if (request.method !== 'GET' || url.origin !== self.location.origin) return
  if (request.mode === 'navigate') {
    event.respondWith(edgeFirst(request, PAGE))
  } else if (url.pathname === '/runtime.json') {
    event.respondWith(edgeFirst(request, url.pathname))
  } else if (CACHED.has(url.pathname)) {
    // Bundle files are content-hashed, so a cached copy is never stale.
    event.respondWith(caches.match(url.pathname).then((hit) => hit ?? fetch(request)))
  }
})

// The edge's answer whenever it gives one, so an update shows at once; the cached shell only
// while the edge cannot be reached (restarting, or the LAN is down).
async function edgeFirst(request, key) {
  try {
    return await fetch(request)
  } catch (error) {
    const hit = await caches.match(key)
    if (hit !== undefined) return hit
    throw error
  }
}
