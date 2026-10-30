import type { ApiRequest, ApiResponse, Transport } from '../src/platform/types'

/** A transport that records requests and answers from `reply`. */
export function fakeTransport(reply: (request: ApiRequest) => Partial<ApiResponse>): Transport & { sent: ApiRequest[] } {
  const sent: ApiRequest[] = []
  return {
    sent,
    async request(request) {
      sent.push(request)
      return { status: 200, headers: {}, body: undefined, ...reply(request) }
    },
  }
}
