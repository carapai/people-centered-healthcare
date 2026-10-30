import { describe, expect, test } from 'vitest'

import { generate, type Document } from '../src/openapi'

const doc = (schemas: Record<string, unknown>, paths: Record<string, unknown> = {}): Document =>
  ({ info: { title: 't', version: '0' }, paths, components: { schemas } }) as Document

describe('the OpenAPI generator', () => {
  test('emits dependencies before the schemas that use them, with optional and nullable fields', () => {
    const out = generate(
      doc({
        Visit: {
          type: 'object',
          required: ['patient', 'count'],
          properties: {
            patient: { $ref: '#/components/schemas/Patient' },
            count: { type: 'integer', format: 'int32' },
            note: { type: ['string', 'null'] },
            fee: { oneOf: [{ type: 'null' }, { type: 'number' }] },
          },
        },
        Patient: { type: 'object', required: ['id'], properties: { id: { type: 'string' } } },
      }),
      'test.json',
    )
    expect(out.indexOf('export const Patient')).toBeLessThan(out.indexOf('export const Visit'))
    expect(out).toContain('"count": integer,')
    expect(out).toContain('"note": z.string().nullable().optional(),')
    expect(out).toContain('"fee": decimal.nullable().optional(),')
    expect(out).toContain("import { decimal, integer } from './numbers'")
  })

  test('fails on constructs it does not support, naming where', () => {
    expect(() => generate(doc({ A: { type: 'object', properties: { x: { not: { type: 'string' } } } } }), 't')).toThrow('A.x: unsupported keywords not')
    expect(() => generate(doc({ A: { allOf: [{ type: 'string' }, { type: 'integer' }] } }), 't')).toThrow('allOf')
    expect(() => generate(doc({ A: { $ref: '#/components/schemas/A' } }), 't')).toThrow('recursive')
    expect(() => generate(doc({ A: { $ref: '#/components/schemas/B' } }), 't')).toThrow('unknown schema B')
    expect(() =>
      generate(doc({}, { '/x': { get: { responses: { '200': { content: { 'application/json': { schema: { type: 'string' } } } } } } } }), 't'),
    ).toThrow('missing operationId')
  })
})
