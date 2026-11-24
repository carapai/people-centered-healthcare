// Reading a Report's CSV export back (#557): what a spreadsheet would read. Kept apart from
// `api.ts`, with no imports, so the end-to-end suite can read a downloaded export with it.

/** Reads CSV text back into rows of fields (RFC 4180): what a spreadsheet would read. */
export function parseCsv(text: string): string[][] {
  const rows: string[][] = []
  let row: string[] = []
  let current = ''
  let quoted = false
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i]
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') {
        current += '"'
        i += 1
      } else if (c === '"') quoted = false
      else current += c
    } else if (c === '"') quoted = true
    else if (c === ',') {
      row.push(current)
      current = ''
    } else if (c === '\r') continue
    else if (c === '\n') {
      row.push(current)
      rows.push(row)
      row = []
      current = ''
    } else current += c
  }
  if (current !== '' || row.length > 0) rows.push([...row, current])
  return rows
}
