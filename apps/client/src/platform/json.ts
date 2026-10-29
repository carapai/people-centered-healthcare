import { isLosslessNumber, parse, stringify } from 'lossless-json'

// FHIR decimals and money must keep their source text (ADR 0052, ADR 0054): numbers are parsed
// as LosslessNumber, never as JS floats.

export function parseJson(text: string): unknown {
  return parse(text)
}

export function stringifyJson(value: unknown): string {
  const text = stringify(value)
  if (text === undefined) {
    throw new TypeError('value cannot be serialized to JSON')
  }
  return text
}

export { isLosslessNumber }
