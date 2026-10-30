import type { AuthStrategy } from './auth'
import type { EventSourcePort, InvalidationHint } from './types'

export interface SseOptions {
  fetch?: typeof fetch
  /** First reconnection delay in ms, until the server sends `retry:`; it doubles up to `maxDelay`. */
  delay?: number
  maxDelay?: number
}

/**
 * The EventSource port over server-sent events (client-application.md § Ports). It reads the
 * stream with fetch rather than `EventSource`, which cannot send the Authorization and DPoP
 * headers. One connection serves every subscriber; it opens with the first and closes with the
 * last, and reconnects with `Last-Event-ID`. A response that is not an event stream (an older
 * server, or the web host's fallback page) stops it.
 */
export function sseEventSource(url: string, auth: AuthStrategy, options: SseOptions = {}): EventSourcePort {
  const fetchImpl = options.fetch ?? fetch
  const listeners = new Set<(hint: InvalidationHint) => void>()
  let abort: AbortController | undefined

  async function run(signal: AbortSignal): Promise<void> {
    const target = new URL(url, globalThis.location?.origin)
    const initial = options.delay ?? 1000
    const maxDelay = options.maxDelay ?? 30_000
    let delay = initial
    let lastEventId = ''
    let nonceRetried = false
    while (!signal.aborted) {
      try {
        const headers = new Headers({ accept: 'text/event-stream', ...(await auth.headers('GET', target)) })
        if (lastEventId !== '') headers.set('last-event-id', lastEventId)
        const response = await fetchImpl(target, { headers, signal, cache: 'no-store' })
        if (auth.retryWithNonce(response) && !nonceRetried) {
          nonceRetried = true
          continue
        }
        nonceRetried = false
        if (!response.ok || response.body === null) {
          if (response.status !== 401 && response.status !== 429 && response.status < 500) return
        } else if (!(response.headers.get('content-type') ?? '').startsWith('text/event-stream')) {
          return
        } else {
          delay = initial
          for await (const event of parseEventStream(response.body)) {
            if (event.id !== undefined) lastEventId = event.id
            if (event.retry !== undefined) delay = event.retry
            const hint = event.data === undefined ? undefined : parseHint(event.data)
            if (hint !== undefined) for (const listener of listeners) listener(hint)
          }
        }
      } catch {
        if (signal.aborted) return
      }
      await sleep(delay, signal)
      delay = Math.min(delay * 2, maxDelay)
    }
  }

  return {
    subscribe(listener) {
      listeners.add(listener)
      if (abort === undefined) {
        abort = new AbortController()
        void run(abort.signal)
      }
      return () => {
        listeners.delete(listener)
        if (listeners.size === 0) {
          abort?.abort()
          abort = undefined
        }
      }
    },
  }
}

export interface ServerSentEvent {
  data?: string
  id?: string
  retry?: number
}

/** The HTML `text/event-stream` format: events end at a blank line; `:` lines are comments. */
export async function* parseEventStream(body: ReadableStream<Uint8Array>): AsyncGenerator<ServerSentEvent> {
  let buffer = ''
  let data: string[] = []
  let event: ServerSentEvent = {}
  const decoder = new TextDecoder()
  const reader = body.getReader()
  try {
    for (;;) {
      const { done, value } = await reader.read()
      buffer += done ? decoder.decode() : decoder.decode(value, { stream: true })
      // A trailing CR may be the first half of CRLF, so it waits for the next chunk.
      const lines = buffer.split(done ? /\r\n|\n|\r/ : /\r\n|\n|\r(?!$)/)
      buffer = done ? '' : (lines.pop() ?? '')
      for (const line of lines) {
        if (line === '') {
          if (data.length > 0) event.data = data.join('\n')
          if (event.data !== undefined || event.id !== undefined || event.retry !== undefined) yield event
          data = []
          event = {}
          continue
        }
        if (line.startsWith(':')) continue
        const colon = line.indexOf(':')
        const field = colon === -1 ? line : line.slice(0, colon)
        const text = colon === -1 ? '' : line.slice(colon + 1).replace(/^ /, '')
        if (field === 'data') data.push(text)
        else if (field === 'id' && !text.includes('\0')) event.id = text
        else if (field === 'retry' && /^\d+$/.test(text)) event.retry = Number(text)
      }
      if (done) return
    }
  } finally {
    reader.releaseLock()
  }
}

/** A hint carries only string identifiers; anything else is ignored. */
function parseHint(data: string): InvalidationHint | undefined {
  let value: unknown
  try {
    value = JSON.parse(data)
  } catch {
    return undefined
  }
  if (typeof value !== 'object' || value === null) return undefined
  const hint: InvalidationHint = {}
  for (const key of ['resourceType', 'id', 'worklist'] as const) {
    const field = (value as Record<string, unknown>)[key]
    if (typeof field === 'string') hint[key] = field
  }
  return hint.resourceType === undefined && hint.worklist === undefined ? undefined : hint
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, ms)
    signal.addEventListener('abort', () => {
      clearTimeout(timer)
      resolve()
    })
  })
}
