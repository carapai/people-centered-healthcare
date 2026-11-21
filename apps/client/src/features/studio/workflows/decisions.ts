// The decision-table editor's model (studio.md § Decision tables; #516), as pure functions.
//
// A table's rows are rules: a name, an outcome, and a condition. The editor shows each condition
// as a tree of groups joined by all, any or not (none of these) over conditions on the
// workflow's declared data requirements, and saves the tree as one FHIRPath expression (`and`,
// `or`, `.not()`) in the row's `when`, so the engine's table keeps its shape. Opening a table
// parses each expression back into its tree; an expression the editor did not write is kept as
// FHIRPath text. Inputs are the declared requirements' paths as FHIRPath reads them
// (`%<requirement>.<path>`), as `health_workflow::decision` names them.
//
// The engine calls a row's outcome `then`, which the client's lint forbids as a property name
// (`no-thenable`): only `readRules` and `writeRules` touch it.

import type { DataRequirement, DecisionTable, Value, WorkflowDefinition } from './model'

export const OPERATORS = ['=', '!=', '<', '<=', '>', '>='] as const
export type Operator = (typeof OPERATORS)[number]
export const JOINS = ['all', 'any', 'not'] as const
export type Join = (typeof JOINS)[number]
export type Literal = boolean | number | string
export type LiteralKind = 'boolean' | 'number' | 'code'

export interface Condition {
  kind: 'condition'
  input: string
  op: Operator
  value: Literal
}
/** A group of conditions joined by all, any or not. `A` is the other conditions a tree may hold
 * (an indicator's dates and memberships, `indicators/conditions.ts`); a decision table's hold none. */
export interface Group<A = never> {
  kind: 'group'
  join: Join
  children: Node<A>[]
}
/** An expression the editor did not write, kept as FHIRPath text. */
export interface Raw {
  kind: 'fhirpath'
  expression: string
}
export type Node<A = never> = Condition | Group<A> | Raw | A

/** How a tree's other conditions are written and read back: `compile` writes one, and `read`
 * recognises one at the start of `rest`, with the length of its text. */
export interface Atoms<A> {
  compile: (atom: A) => string
  read: (rest: string) => { atom: A; length: number } | null
}

const OWN_KINDS = new Set(['condition', 'group', 'fhirpath'])

/** Whether `node` is one of the tree's other conditions. */
export const isAtom = <A>(node: Node<A>): node is A => !OWN_KINDS.has((node as { kind: string }).kind)

export interface Rule {
  name: string
  tree: Group
  outcome: Value
}

export const kindOf = (value: Literal): LiteralKind => (typeof value === 'boolean' ? 'boolean' : typeof value === 'number' ? 'number' : 'code')

/** The operators a condition on a value of `kind` may use. */
export const operatorsFor = (kind: LiteralKind): readonly Operator[] => (kind === 'number' ? OPERATORS : ['=', '!='])

// ---------------------------------------------------------------------------------------------
// Inputs

const IDENTIFIER = /^[A-Za-z_]\w*$/

/** The inputs `requirement` gives: one per path, as `health_workflow::DataRequirement::inputs`. */
export function requirementInputs(requirement: DataRequirement): string[] {
  const out: string[] = []
  for (const path of requirement.paths ?? []) {
    let rest = path
    if (path === requirement.resourceType) continue
    if (path.startsWith(`${requirement.resourceType}.`)) rest = path.slice(requirement.resourceType.length + 1)
    const named = `%${requirement.name}.${rest}`
    if (isInput(named)) out.push(named)
  }
  return out
}

export function isInput(text: string): boolean {
  if (!text.startsWith('%')) return false
  const parts = text.slice(1).split('.')
  return parts.length >= 2 && parts.every((p) => IDENTIFIER.test(p))
}

/** Every input the definition's data requirements give. */
export const inputsOf = (definition: WorkflowDefinition): string[] => [...new Set((definition.dataRequirements ?? []).flatMap(requirementInputs))]

// ---------------------------------------------------------------------------------------------
// Compiling a tree to FHIRPath

const literalText = (value: Literal): string => (typeof value === 'string' ? `'${value.replaceAll('\\', '\\\\').replaceAll("'", String.raw`\'`)}'` : String(value))

/** A node as FHIRPath. An empty group holds. Other conditions are written by `atoms`. */
export function compile<A = never>(node: Node<A>, atoms?: Atoms<A>): string {
  if (isAtom(node)) {
    if (atoms === undefined) throw new Error('a condition the tree cannot write')
    return atoms.compile(node)
  }
  const own = node as Condition | Group<A> | Raw
  if (own.kind === 'condition') return `${own.input} ${own.op} ${literalText(own.value)}`
  if (own.kind === 'fhirpath') return own.expression
  if (own.children.length === 0) return 'true'
  const many = own.children.length > 1 || own.join === 'not'
  const parts = own.children.map((child) => {
    const text = compile(child, atoms)
    const kind = (child as { kind: string }).kind
    const nested = (kind === 'group' && (child as Group<A>).join !== 'not' && (child as Group<A>).children.length > 1) || kind === 'fhirpath'
    return many && nested ? `(${text})` : text
  })
  if (own.join === 'all') return parts.join(' and ')
  if (own.join === 'any') return parts.join(' or ')
  return `(${parts.join(' or ')}).not()`
}

// ---------------------------------------------------------------------------------------------
// Parsing FHIRPath back into a tree: only what `compile` writes.

type Token<A> =
  | { t: 'open' | 'close' | 'not' | 'and' | 'or' | 'true' | 'false' }
  | { t: 'op'; v: Operator }
  | { t: 'input'; v: string }
  | { t: 'number'; v: number }
  | { t: 'string'; v: string }
  | { t: 'atom'; v: A }

function tokenize<A>(text: string, atoms?: Atoms<A>): Token<A>[] | null {
  const out: Token<A>[] = []
  let at = 0
  while (at < text.length) {
    const rest = text.slice(at)
    const space = /^\s+/.exec(rest)
    if (space !== null) {
      at += space[0].length
      continue
    }
    const atom = atoms?.read(rest) ?? null
    if (atom !== null) {
      out.push({ t: 'atom', v: atom.atom })
      at += atom.length
      continue
    }
    if (rest.startsWith('.not()')) {
      out.push({ t: 'not' })
      at += 6
      continue
    }
    const char = rest[0]
    if (char === '(' || char === ')') {
      out.push({ t: char === '(' ? 'open' : 'close' })
      at += 1
      continue
    }
    const op = /^(<=|>=|!=|=|<|>)/.exec(rest)
    if (op !== null) {
      out.push({ t: 'op', v: op[0] as Operator })
      at += op[0].length
      continue
    }
    const input = /^%[A-Za-z_]\w*(\.[A-Za-z_]\w*)+/.exec(rest)
    if (input !== null) {
      if (rest[input[0].length] === '(') return null
      out.push({ t: 'input', v: input[0] })
      at += input[0].length
      continue
    }
    const number = /^-?\d+(?![\w.])/.exec(rest)
    if (number !== null) {
      out.push({ t: 'number', v: Number(number[0]) })
      at += number[0].length
      continue
    }
    if (char === "'") {
      let value = ''
      let n = 1
      for (; n < rest.length && rest[n] !== "'"; n++) {
        if (rest[n] === '\\') {
          const next = rest[n + 1]
          if (next !== "'" && next !== '\\') return null
          value += next
          n++
        } else value += rest[n]
      }
      if (n >= rest.length) return null
      out.push({ t: 'string', v: value })
      at += n + 1
      continue
    }
    const word = /^(and|or|true|false)\b/.exec(rest)
    if (word !== null) {
      out.push({ t: word[0] as 'and' | 'or' | 'true' | 'false' })
      at += word[0].length
      continue
    }
    return null
  }
  return out
}

class Parser<A> {
  at = 0
  constructor(readonly tokens: Token<A>[]) {}
  peek = () => this.tokens[this.at]
  next = () => this.tokens[this.at++]

  expr(): Node<A> | null {
    const first = this.and()
    if (first === null) return null
    const children = [first]
    while (this.peek()?.t === 'or') {
      this.next()
      const next = this.and()
      if (next === null) return null
      children.push(next)
    }
    return children.length === 1 ? first : { kind: 'group', join: 'any', children }
  }

  and(): Node<A> | null {
    const first = this.unary()
    if (first === null) return null
    const children = [first]
    while (this.peek()?.t === 'and') {
      this.next()
      const next = this.unary()
      if (next === null) return null
      children.push(next)
    }
    return children.length === 1 ? first : { kind: 'group', join: 'all', children }
  }

  unary(): Node<A> | null {
    const token = this.peek()
    if (token?.t === 'atom') {
      this.next()
      return token.v
    }
    if (token?.t === 'open') {
      this.next()
      const inner = this.expr()
      if (inner === null || this.next()?.t !== 'close') return null
      if (this.peek()?.t === 'not') {
        this.next()
        const any = !isAtom(inner) && inner.kind === 'group' && inner.join === 'any'
        return {
          kind: 'group',
          join: 'not',
          children: any ? (inner as Group<A>).children : [inner],
        }
      }
      // A parenthesised group stays a group of its own.
      return inner
    }
    if (token?.t === 'true') {
      this.next()
      return { kind: 'group', join: 'all', children: [] }
    }
    if (token?.t !== 'input') return null
    this.next()
    const op = this.next()
    const value = this.next()
    if (op?.t !== 'op' || value === undefined) return null
    const read = value.t === 'number' || value.t === 'string' ? value.v : value.t === 'true' ? true : value.t === 'false' ? false : undefined
    if (read === undefined) return null
    if (typeof read !== 'number' && op.v !== '=' && op.v !== '!=') return null
    return { kind: 'condition', input: token.v, op: op.v, value: read }
  }
}

/** The tree `expression` holds: what `compile` wrote, or the text kept as FHIRPath. Other
 * conditions are read by `atoms`. */
export function parse<A = never>(expression: string, atoms?: Atoms<A>): Group<A> {
  const tokens = tokenize(expression, atoms)
  const parser = tokens === null ? null : new Parser<A>(tokens)
  const node = parser?.expr() ?? null
  if (node === null || parser === null || parser.at !== parser.tokens.length)
    return {
      kind: 'group',
      join: 'all',
      children: [{ kind: 'fhirpath', expression }],
    }
  return !isAtom(node) && node.kind === 'group' ? (node as Group<A>) : { kind: 'group', join: 'all', children: [node] }
}

// ---------------------------------------------------------------------------------------------
// The engine's rows, at the boundary

type EngineRow = DecisionTable['rows'][number]

/** A table's rows as rules: each row's conditions (one expression when the editor wrote it) as
 * one tree. */
export function readRules(table: DecisionTable): Rule[] {
  return table.rows.map((row, n) => {
    const trees = row.when.map((when) => parse(when))
    const tree: Group =
      trees.length === 1 && trees[0] !== undefined
        ? trees[0]
        : {
            kind: 'group',
            join: 'all',
            children: trees.map((t) => (t.children.length === 1 && t.join === 'all' && t.children[0] !== undefined ? t.children[0] : t)),
          }
    // oxlint-disable-next-line unicorn/no-thenable
    const outcome = (row as { then: Value }).then
    return { name: row.name ?? `Rule ${n + 1}`, tree, outcome }
  })
}

/** Rules as the engine's rows: each tree saved as one expression. */
export function writeRules(rules: Rule[]): EngineRow[] {
  return rules.map((rule) => {
    const name = rule.name.trim()
    return {
      ...(name === '' ? {} : { name }),
      when: [compile(rule.tree)],
      // oxlint-disable-next-line unicorn/no-thenable -- the engine's name for a row's outcome
      then: rule.outcome,
    }
  })
}

/** `definition` with its `index`-th table replaced by `table`. */
export function withTable(definition: WorkflowDefinition, index: number, table: DecisionTable): WorkflowDefinition {
  return {
    ...definition,
    decisionTables: (definition.decisionTables ?? []).map((t, n) => (n === index ? table : t)),
  }
}

/** `definition` with a new, empty table, and its index. */
export function addTable(definition: WorkflowDefinition): {
  definition: WorkflowDefinition
  index: number
} {
  const tables = definition.decisionTables ?? []
  const ids = new Set(tables.map((t) => t.id))
  let id = 'table'
  for (let n = 2; ids.has(id); n++) id = `table-${n}`
  return {
    definition: {
      ...definition,
      decisionTables: [...tables, { id, rows: [], default: null }],
    },
    index: tables.length,
  }
}

// ---------------------------------------------------------------------------------------------
// Editing a tree

/** `root` with the node at `path` (child indexes) replaced by what `f` returns; null removes it. */
export function updateNode(root: Group, path: number[], f: (node: Node) => Node | null): Group {
  const walk = (node: Node, rest: number[]): Node | null => {
    if (rest.length === 0) return f(node)
    if (node.kind !== 'group') return node
    const [head, ...tail] = rest
    return {
      ...node,
      children: node.children.flatMap((child, n) => (n === head ? [walk(child, tail)].filter((c): c is Node => c !== null) : [child])),
    }
  }
  const next = walk(root, path)
  return next !== null && next.kind === 'group' ? next : root
}

/** Every node of `tree` with its path, depth first. */
export function nodes(tree: Node, path: number[] = []): { node: Node; path: number[] }[] {
  return [{ node: tree, path }, ...(tree.kind === 'group' ? tree.children.flatMap((child, n) => nodes(child, [...path, n])) : [])]
}

/** The inputs a tree's conditions read. */
export const inputsRead = (tree: Node): string[] => [...new Set(nodes(tree).flatMap(({ node }) => (node.kind === 'condition' ? [node.input] : [])))]

// ---------------------------------------------------------------------------------------------
// Sample values

/** Each input's kind, from the first condition on it, `code` otherwise. */
export function inputKinds(rules: Rule[], inputs: string[]): Record<string, LiteralKind> {
  const out: Record<string, LiteralKind> = {}
  for (const rule of rules) for (const { node } of nodes(rule.tree)) if (node.kind === 'condition' && out[node.input] === undefined) out[node.input] = kindOf(node.value)
  for (const input of inputs) out[input] ??= 'code'
  return out
}

/** The sample values coverage tries for each declared input a condition reads: both flags, each
 * number a condition names with its neighbours, and each code a condition names. */
export function samples(rules: Rule[], inputs: string[]): Record<string, Literal[]> {
  const declared = new Set(inputs)
  const out = new Map<string, Literal[]>()
  const add = (input: string, ...values: Literal[]) => {
    const list = out.get(input) ?? []
    for (const value of values) if (!list.includes(value)) list.push(value)
    out.set(input, list)
  }
  for (const rule of rules)
    for (const { node } of nodes(rule.tree)) {
      if (node.kind !== 'condition' || !declared.has(node.input)) continue
      if (typeof node.value === 'boolean') add(node.input, true, false)
      else if (typeof node.value === 'number') add(node.input, node.value - 1, node.value, node.value + 1)
      else add(node.input, node.value)
    }
  const sorted = (values: Literal[]) => {
    const copy = [...values]
    // oxlint-disable-next-line unicorn/no-array-sort -- a copy, sorted in place
    copy.sort((a, b) => Number(a) - Number(b))
    return copy
  }
  return Object.fromEntries([...out].map(([input, values]) => [input, typeof values[0] === 'number' ? sorted(values) : values]))
}

/** How many combinations `samples` give. */
export const combinations = (sampled: Record<string, Literal[]>) => Object.values(sampled).reduce((n, values) => n * values.length, 1)

/** A value as the editor shows it. */
export const shown = (value: Literal | null | undefined): string => (value === null || value === undefined ? '' : String(value))

/** A blank value of `kind`. */
export const blank = (kind: LiteralKind): Literal => (kind === 'boolean' ? true : kind === 'number' ? 0 : '')

/** `text` read as a value of `kind`. */
export function literal(kind: LiteralKind, text: string): Literal {
  if (kind === 'boolean') return text === 'true'
  if (kind === 'number') return Number.isFinite(Number(text)) ? Math.trunc(Number(text)) : 0
  return text
}

/** The test case *Save as test* writes into the package's `tests/` (`health_workflow::decision::TableTest`). */
export function testCase(definition: WorkflowDefinition, table: string, name: string, inputs: Record<string, Literal>, expect: Value) {
  return {
    type: 'decision-table-test',
    name,
    definition: `${definition.url}|${definition.version}`,
    table,
    inputs,
    expect,
  }
}

/** A file name for a test case: `tests/<table>-<name>.json`, lower case, dashes for the rest. */
export function testPath(table: string, name: string, taken: Set<string>): string {
  const slug = `${table}-${name}`
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, '-')
    .replaceAll(/^-|-$/g, '')
  let path = `tests/${slug}.json`
  for (let n = 2; taken.has(path); n++) path = `tests/${slug}-${n}.json`
  return path
}
