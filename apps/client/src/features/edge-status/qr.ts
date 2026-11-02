import { encode } from 'uqr'

/**
 * `text` as a QR code: an SVG path of its dark modules in a `size`-unit square, with the
 * four-module quiet zone scanners need. Drawn as a path, not injected markup.
 */
export function qrPath(text: string): { size: number; path: string } {
  const { size, data } = encode(text, { ecc: 'M', border: 4 })
  const parts: string[] = []
  data.forEach((row, y) => {
    row.forEach((dark, x) => {
      if (dark) parts.push(`M${x} ${y}h1v1h-1z`)
    })
  })
  return { size, path: parts.join('') }
}
