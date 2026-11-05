import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, test } from 'vitest'

import { type Fixture, evaluate, parseLossless } from '../src'

// The shared FHIRPath conformance suite against fhirpath.js (ADRs 0038, 0054). The same fixtures
// run against the server's engine in health-fhir-validation's tests/fhirpath_conformance.rs.
const dir = join(import.meta.dirname, '../fixtures')
const fixtures = readdirSync(dir)
  .filter((file) => file.endsWith('.json'))
  .sort()
  .map((file) => {
    const text = readFileSync(join(dir, file), 'utf8')
    // Expectations are plain JSON; the resource is parsed losslessly, as a form's response is.
    const plain = JSON.parse(text) as Fixture
    const lossless = parseLossless(text) as { resource: unknown }
    return { ...plain, resource: lossless.resource }
  })

describe('fhirpath.js in lossless precise mode', () => {
  test('there are fixtures', () => {
    expect(fixtures.length).toBeGreaterThan(0)
  })
  for (const fixture of fixtures) {
    describe(fixture.group, () => {
      for (const c of fixture.cases) {
        test(c.name, () => {
          expect(evaluate(fixture.resource, c.expression)).toEqual(c.expected)
        })
      }
    })
  }
})
