import { parse } from '@formatjs/icu-messageformat-parser'
import { expect, test } from 'vitest'

import { englishMessages, messages } from '../src'

test('every message is valid ICU with a unique ds.* id and a description', () => {
  const descriptors = Object.values(messages)
  const ids = descriptors.map((m) => m.id)
  expect(new Set(ids).size).toBe(ids.length)
  for (const m of descriptors) {
    expect(m.id).toMatch(/^ds\.[a-zA-Z]+\.[a-zA-Z]+$/)
    expect(m.description, m.id).toBeTruthy()
    expect(() => parse(m.defaultMessage as string), m.id).not.toThrow()
  }
  expect(Object.keys(englishMessages)).toEqual(ids)
})
