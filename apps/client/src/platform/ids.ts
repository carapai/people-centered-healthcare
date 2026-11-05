/**
 * A new resource id: a UUIDv7 (RFC 9562), as every client-created resource needs (ADR 0013).
 * Its first 48 bits are the Unix time in milliseconds and the rest is random; the timestamp is
 * never relied on for ordering (identifiers.md).
 */
export function newResourceId(now: number = Date.now()): string {
  const bytes = new Uint8Array(16)
  crypto.getRandomValues(bytes)
  let time = now
  for (let i = 5; i >= 0; i -= 1) {
    bytes[i] = time % 256
    time = Math.floor(time / 256)
  }
  bytes[6] = 0x70 | ((bytes[6] ?? 0) & 0x0f)
  bytes[8] = 0x80 | ((bytes[8] ?? 0) & 0x3f)
  const hex = [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}
