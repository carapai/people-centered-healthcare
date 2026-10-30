import { isLosslessNumber } from 'lossless-json'
import { expect, test } from 'vitest'

import { parseJson, stringifyJson } from '../src/platform/json'

test('decimals keep their source text', () => {
  const text = '{"value":1.50,"precise":0.1000000000000000000000000001,"big":12345678901234567890,"exp":1.0e2}'
  const parsed = parseJson(text) as Record<string, unknown>
  expect(isLosslessNumber(parsed['value'])).toBe(true)
  expect(String(parsed['value'])).toBe('1.50')
  expect(stringifyJson(parsed)).toBe(text)
})

test('a value JSON cannot represent is refused', () => {
  expect(() => stringifyJson(undefined)).toThrow(TypeError)
})
