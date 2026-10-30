import { describe, expect, test, vi } from 'vitest'

import type { AuthStrategy } from '../src/platform/auth'
import { parseEventStream, sseEventSource } from '../src/platform/events'
import type { InvalidationHint } from '../src/platform/types'

function stream(...chunks: string[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder()
  return new ReadableStream({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(encoder.encode(chunk))
      controller.close()
    },
  })
}

async function collect(body: ReadableStream<Uint8Array>) {
  const events = []
  for await (const event of parseEventStream(body)) events.push(event)
  return events
}

const auth: AuthStrategy = { headers: async () => ({ authorization: 'Bearer t' }), retryWithNonce: () => false }

describe('parseEventStream', () => {
  test('events split across chunks and line endings', async () => {
    expect(await collect(stream(': keep-alive\n\nid: 1\nda', 'ta: {"a":\r', '\ndata: 1}\r\n\r', '\nretry: 50\n\ndata:x\rdata\r\r'))).toEqual([
      { id: '1', data: '{"a":\n1}' },
      { retry: 50 },
      { data: 'x\n' },
    ])
  })

  test('an unterminated last event is dropped', async () => {
    expect(await collect(stream('data: a\n\ndata: b'))).toEqual([{ data: 'a' }])
  })
})

describe('sseEventSource', () => {
  test('delivers hints, reconnects with Last-Event-ID and closes with the last subscriber', async () => {
    const requests: Headers[] = []
    let calls = 0
    const fetchImpl: typeof fetch = async (_, init) => {
      requests.push(new Headers(init?.headers))
      calls += 1
      const body =
        calls === 1
          ? 'id: 7\ndata: {"resourceType":"Patient","id":"p1"}\n\ndata: not json\n\ndata: {"worklist":"triage","extra":1}\n\n'
          : ': idle\n'
      return new Response(stream(body), { headers: { 'content-type': 'text/event-stream' } })
    }
    const hints: InvalidationHint[] = []
    const events = sseEventSource('https://edge.example/api/events', auth, { fetch: fetchImpl, delay: 1 })
    const unsubscribe = events.subscribe((hint) => hints.push(hint))
    await vi.waitFor(() => expect(calls).toBeGreaterThanOrEqual(2))
    unsubscribe()
    expect(hints).toEqual([{ resourceType: 'Patient', id: 'p1' }, { worklist: 'triage' }])
    expect(requests[0]?.get('authorization')).toBe('Bearer t')
    expect(requests[0]?.get('accept')).toBe('text/event-stream')
    expect(requests[1]?.get('last-event-id')).toBe('7')
    const after = calls
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(calls).toBe(after)
  })

  test('stops on a response that is not an event stream', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response('<html>', { headers: { 'content-type': 'text/html' } }))
    const unsubscribe = sseEventSource('https://edge.example/api/events', auth, { fetch: fetchImpl, delay: 1 }).subscribe(() => {})
    await new Promise((resolve) => setTimeout(resolve, 20))
    unsubscribe()
    expect(fetchImpl).toHaveBeenCalledTimes(1)
  })

  test('retries a server error', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response(null, { status: 503 }))
    const unsubscribe = sseEventSource('https://edge.example/api/events', auth, { fetch: fetchImpl, delay: 1, maxDelay: 2 }).subscribe(() => {})
    await vi.waitFor(() => expect(fetchImpl.mock.calls.length).toBeGreaterThanOrEqual(3))
    unsubscribe()
  })
})
