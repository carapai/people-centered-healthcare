// Generates @health/api-client's Zod schemas and types from a runtime's OpenAPI document, which
// the Rust handlers produce with utoipa (client-application.md § Types; ADR 0036).
//
//   bun src/openapi.ts <openapi.json> <out.ts>
//
// It supports the JSON Schema that utoipa emits for our DTOs: objects, string enums, strings,
// integers, numbers, booleans, arrays, maps, `$ref`, `oneOf`/`anyOf` (a `null` member makes the
// schema nullable) and single-member `allOf`. Anything else fails the generation, so an
// unsupported DTO is caught here rather than mis-typed. Integers become JS numbers (checked
// safe); other numbers stay LosslessNumber (ADR 0052). Types are `z.infer` of the schemas.

import { readFileSync, writeFileSync } from 'node:fs'
import { relative } from 'node:path'

type Schema = {
  $ref?: string
  type?: string | string[]
  enum?: unknown[]
  const?: unknown
  format?: string
  description?: string
  properties?: Record<string, Schema>
  required?: string[]
  additionalProperties?: Schema | boolean
  propertyNames?: Schema
  items?: Schema
  oneOf?: Schema[]
  anyOf?: Schema[]
  allOf?: Schema[]
}

interface Operation {
  operationId?: string
  responses?: Record<string, { content?: Record<string, { schema?: Schema }> }>
}

export interface Document {
  info: { title: string; version: string }
  paths?: Record<string, Record<string, Operation>>
  components?: { schemas?: Record<string, Schema> }
}

const methods = ['get', 'put', 'post', 'delete', 'patch'] as const
// Keys with no effect on the Zod schema.
const ignored = new Set(['description', 'format', 'propertyNames', 'title', 'example', 'examples', 'default', 'minimum', 'maximum', 'readOnly', 'writeOnly', 'deprecated'])

const refName = (ref: string, where: string) => {
  const prefix = '#/components/schemas/'
  if (!ref.startsWith(prefix)) throw new Error(`${where}: unsupported $ref ${ref}`)
  return ref.slice(prefix.length)
}

function comment(text: string | undefined, indent = ''): string[] {
  if (text === undefined || text === '') return []
  return [`${indent}/** ${text.replaceAll('*/', '*\\/').replaceAll('\n', ' ')} */`]
}

/** The Zod expression for `schema`; `refs` collects the component names it uses. */
function zod(schema: Schema, where: string, refs: Set<string>): string {
  const unknownKeys = Object.keys(schema).filter((k) => !ignored.has(k) && !['$ref', 'type', 'enum', 'const', 'properties', 'required', 'additionalProperties', 'items', 'oneOf', 'anyOf', 'allOf'].includes(k))
  if (unknownKeys.length > 0) throw new Error(`${where}: unsupported keywords ${unknownKeys.join(', ')}`)
  if (schema.$ref !== undefined) {
    const name = refName(schema.$ref, where)
    refs.add(name)
    return name
  }
  const members = schema.oneOf ?? schema.anyOf
  if (members !== undefined) {
    const nonNull = members.filter((m) => m.type !== 'null')
    const inner = nonNull.map((m, i) => zod(m, `${where}.oneOf[${i}]`, refs))
    const union = inner.length === 1 ? (inner[0] as string) : `z.union([${inner.join(', ')}])`
    return nonNull.length < members.length ? `${union}.nullable()` : union
  }
  if (schema.allOf !== undefined) {
    if (schema.allOf.length !== 1) throw new Error(`${where}: allOf with ${schema.allOf.length} members is unsupported`)
    return zod(schema.allOf[0] as Schema, `${where}.allOf[0]`, refs)
  }
  if (schema.const !== undefined) return `z.literal(${JSON.stringify(schema.const)})`
  const types = Array.isArray(schema.type) ? schema.type : schema.type === undefined ? [] : [schema.type]
  const nullable = types.includes('null')
  const rest = types.filter((t) => t !== 'null')
  if (rest.length !== 1) throw new Error(`${where}: unsupported type ${JSON.stringify(schema.type)}`)
  let expression: string
  switch (rest[0]) {
    case 'string':
      if (schema.enum !== undefined) {
        if (!schema.enum.every((v) => typeof v === 'string')) throw new Error(`${where}: non-string enum`)
        expression = `z.enum([${schema.enum.map((v) => JSON.stringify(v)).join(', ')}])`
      } else {
        expression = 'z.string()'
      }
      break
    case 'integer':
      expression = 'integer'
      break
    case 'number':
      expression = 'decimal'
      break
    case 'boolean':
      expression = 'z.boolean()'
      break
    case 'array':
      if (schema.items === undefined) throw new Error(`${where}: array without items`)
      expression = `z.array(${zod(schema.items, `${where}.items`, refs)})`
      break
    case 'object':
      expression = object(schema, where, refs)
      break
    default:
      throw new Error(`${where}: unsupported type ${String(rest[0])}`)
  }
  return nullable ? `${expression}.nullable()` : expression
}

function object(schema: Schema, where: string, refs: Set<string>): string {
  const properties = Object.entries(schema.properties ?? {})
  const extra = schema.additionalProperties
  if (properties.length === 0 && extra !== undefined && extra !== false) {
    const value = extra === true ? 'z.unknown()' : zod(extra, `${where}.additionalProperties`, refs)
    return `z.record(z.string(), ${value})`
  }
  if (extra !== undefined && extra !== false) throw new Error(`${where}: properties with additionalProperties are unsupported`)
  const required = new Set(schema.required ?? [])
  const lines = properties.map(([name, property]) => {
    const value = zod(property, `${where}.${name}`, refs)
    return [...comment(property.description, '  '), `  ${JSON.stringify(name)}: ${required.has(name) ? value : `${value}.optional()`},`].join('\n')
  })
  return lines.length === 0 ? 'z.object({})' : `z.object({\n${lines.join('\n')}\n})`
}

/** The generated module for `document`. */
export function generate(document: Document, source: string): string {
  const schemas = document.components?.schemas ?? {}
  const compiled = new Map<string, { expression: string; refs: Set<string>; description: string | undefined }>()
  for (const [name, schema] of Object.entries(schemas)) {
    const refs = new Set<string>()
    compiled.set(name, { expression: zod(schema, name, refs), refs, description: schema.description })
  }
  // Dependencies first; a cycle would need z.lazy, which no DTO needs yet.
  const order: string[] = []
  const state = new Map<string, 'visiting' | 'done'>()
  const visit = (name: string, path: string[]) => {
    if (state.get(name) === 'done') return
    if (state.get(name) === 'visiting') throw new Error(`recursive schemas are unsupported: ${[...path, name].join(' → ')}`)
    const entry = compiled.get(name)
    if (entry === undefined) throw new Error(`${path.at(-1) ?? 'document'}: unknown schema ${name}`)
    state.set(name, 'visiting')
    for (const ref of [...entry.refs].toSorted()) visit(ref, [...path, name])
    state.set(name, 'done')
    order.push(name)
  }
  for (const name of [...compiled.keys()].toSorted()) visit(name, [])

  const out: string[] = [
    `// Generated by tools/codegen/src/openapi.ts from ${source} (${document.info.title} ${document.info.version}).`,
    '// Do not edit; run `bun run generate` in tools/codegen.',
    '',
    "import { z } from 'zod'",
  ]
  for (const name of order) {
    const entry = compiled.get(name) as { expression: string; description: string | undefined }
    out.push('', ...comment(entry.description), `export const ${name} = ${entry.expression}`, `export type ${name} = z.infer<typeof ${name}>`)
  }

  const expressions = [...compiled.values()].map((c) => c.expression)
  const operations: string[] = []
  for (const [path, item] of Object.entries(document.paths ?? {}).toSorted(([a], [b]) => a.localeCompare(b))) {
    for (const method of methods) {
      const operation = item[method]
      if (operation === undefined) continue
      const where = `${method.toUpperCase()} ${path}`
      if (operation.operationId === undefined) throw new Error(`${where}: missing operationId`)
      const responses = Object.entries(operation.responses ?? {}).map(([status, response]) => {
        const media = Object.keys(response.content ?? {})
        if (media.length === 0) return `      ${status}: null,`
        const schema = response.content?.['application/json']?.schema
        if (media.length !== 1 || schema === undefined) throw new Error(`${where} ${status}: only application/json responses are supported`)
        const expression = zod(schema, `${where} ${status}`, new Set<string>())
        expressions.push(expression)
        return `      ${status}: ${expression},`
      })
      operations.push(`  ${operation.operationId}: {\n    method: '${method.toUpperCase()}',\n    path: '${path}',\n    responses: {\n${responses.join('\n')}\n    },\n  },`)
    }
  }
  const numbers = ['decimal', 'integer'].filter((n) => expressions.some((e) => new RegExp(`\\b${n}\\b`).test(e)))
  if (numbers.length > 0) out.splice(4, 0, '', `import { ${numbers.join(', ')} } from './numbers'`)
  out.push('', '/** Each operation: its method, path and response schemas by status (`null`: no body). */', `export const operations = {\n${operations.join('\n')}\n} as const`, '')
  return out.join('\n')
}

if (import.meta.main) {
  const [input, output] = process.argv.slice(2)
  if (input === undefined || output === undefined) throw new Error('usage: bun src/openapi.ts <openapi.json> <out.ts>')
  writeFileSync(output, generate(JSON.parse(readFileSync(input, 'utf8')) as Document, relative(process.cwd(), input)))
  console.log(`openapi: ${input} → ${output}`)
}
