// fhirpath.js in lossless precise mode (ADR 0054), as the client's live forms run it, and the
// normal form both engines' results are compared in (ADR 0038).
import fhirpath from 'fhirpath'
import r5 from 'fhirpath/fhir-context/r5'
import { parse } from 'lossless-json'

/** A FHIRPath result item as the shared fixtures write it. */
export type Item =
  | { boolean: boolean }
  | { number: string }
  | { text: string }
  | { quantity: { value: string; unit: string | null } }

/** A fixture group: a resource and the cases evaluated against it. */
export interface Fixture {
  group: string
  description: string
  resource: unknown
  cases: { name: string; expression: string; expected: Item[] }[]
}

const { FP_Decimal } = fhirpath as unknown as { FP_Decimal: { getDecimal(raw: string): unknown } }

/**
 * Parses resource JSON losslessly (the fhirpath.js README's number parser): each decimal becomes
 * fhirpath.js's own exact decimal from its source text (`1.50` stays `1.50`), which plain
 * `JSON.parse` loses; integers stay numbers, so `valueInteger` keeps its type.
 */
export function parseLossless(text: string): unknown {
  return parse(text, null, (raw: string) =>
    /^-?\d+$/.test(raw) && Number.isSafeInteger(Number(raw)) ? Number(raw) : FP_Decimal.getDecimal(raw),
  )
}

// fhirpath.js's precise number type; in precise mode integers are returned as it too, so both
// engines' numbers compare as their exact text.
const precise = (value: unknown) => (value as { constructor?: { name?: string } })?.constructor?.name ?? ''

function normal(value: unknown): Item {
  if (typeof value === 'boolean') return { boolean: value }
  if (typeof value === 'string') return { text: value }
  if (typeof value === 'number') return { number: String(value) }
  const kind = precise(value)
  if (kind.includes('FP_Decimal')) return { number: String(value) }
  if (kind.includes('FP_Quantity')) {
    const quantity = value as { value: unknown; unit: string | null }
    return { quantity: { value: String(quantity.value), unit: quantity.unit ?? null } }
  }
  return { text: String(value) }
}

/** Evaluates `expression` against `resource` with fhirpath.js in lossless precise mode. */
export function evaluate(resource: unknown, expression: string): Item[] {
  const result = fhirpath.evaluate(resource as object, expression, {}, r5, {
    preciseMath: true,
    keepDecimalTypes: true,
  }) as unknown[]
  return result.map(normal)
}
