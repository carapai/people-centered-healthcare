// PROTOTYPE (#513), throwaway: a decision table as `health_workflow::definition::DecisionTable`
// holds it (rows of FHIRPath conditions that must all hold, an outcome, first match wins, an
// optional default), a tiny evaluator for the simple conditions the screens produce, a coverage
// check over sample inputs, and test cases. The real evaluation is the server's FHIRPath engine.

export type Input = { id: string; label: string; path: string; type: 'code' | 'number' | 'boolean'; values?: string[] | undefined; samples?: number[] | undefined }
export type Op = '=' | '!=' | '<' | '<=' | '>' | '>='
export type Condition = { input: string; op: Op; value: string }
export type Row = { when: Condition[]; outcome: string }
export type Table = { id: string; title: string; inputs: Input[]; rows: Row[]; default: string | null; outcomes: string[] }
export type Case = { name: string; inputs: Record<string, string>; expect: string }

export const TABLE: Table = {
  id: 'triage-route',
  title: 'Route after triage',
  inputs: [
    { id: 'category', label: 'Triage category', path: '%triage.category', type: 'code', values: ['emergency', 'urgent', 'routine'] },
    { id: 'age', label: 'Age (years)', path: '%patient.age()', type: 'number', samples: [0, 3, 10, 30, 70] },
    { id: 'danger', label: 'Danger signs', path: '%triage.dangerSigns.exists()', type: 'boolean' },
    { id: 'pregnant', label: 'Pregnant', path: '%patient.pregnant', type: 'boolean' },
  ],
  rows: [
    { when: [{ input: 'category', op: '=', value: 'emergency' }], outcome: 'bypass' },
    {
      when: [
        { input: 'age', op: '<', value: '5' },
        { input: 'danger', op: '=', value: 'true' },
      ],
      outcome: 'bypass',
    },
    {
      when: [
        { input: 'pregnant', op: '=', value: 'true' },
        { input: 'category', op: '=', value: 'urgent' },
      ],
      outcome: 'bypass',
    },
    { when: [{ input: 'category', op: '=', value: 'routine' }], outcome: 'normal' },
    {
      when: [
        { input: 'category', op: '=', value: 'routine' },
        { input: 'age', op: '<', value: '1' },
      ],
      outcome: 'bypass',
    },
  ],
  default: null,
  outcomes: ['bypass', 'normal'],
}

export const CASES: Case[] = [
  { name: 'Emergency adult', inputs: { category: 'emergency', age: '40', danger: 'false', pregnant: 'false' }, expect: 'bypass' },
  { name: 'Child with danger signs', inputs: { category: 'routine', age: '3', danger: 'true', pregnant: 'false' }, expect: 'bypass' },
  { name: 'Urgent adult, not pregnant', inputs: { category: 'urgent', age: '30', danger: 'false', pregnant: 'false' }, expect: 'normal' },
  { name: 'Routine infant', inputs: { category: 'routine', age: '0', danger: 'false', pregnant: 'false' }, expect: 'bypass' },
]

const literal = (input: Input | undefined, value: string) => (input?.type === 'code' ? `'${value}'` : value)
export const conditionText = (t: Table, c: Condition) => {
  const input = t.inputs.find((i) => i.id === c.input)
  return `${input?.path ?? c.input} ${c.op} ${literal(input, c.value)}`
}
export const rowText = (t: Table, r: Row) => r.when.map((c) => conditionText(t, c)).join(' and ')

/** Parses `<path> <op> <literal>` back into a condition, for the text editor; null when it cannot. */
export function parseCondition(t: Table, text: string): Condition | null {
  const m = /^\s*(\S+)\s*(!=|<=|>=|=|<|>)\s*'?([^']*?)'?\s*$/.exec(text)
  if (!m) return null
  const input = t.inputs.find((i) => i.path === m[1])
  if (!input || !m[2] || m[3] === undefined) return null
  return { input: input.id, op: m[2] as Op, value: m[3] }
}

export function holds(t: Table, c: Condition, sample: Record<string, string>): boolean {
  const input = t.inputs.find((i) => i.id === c.input)
  const raw = sample[c.input]
  if (raw === undefined || !input) return false
  if (input.type === 'number') {
    const a = Number(raw)
    const b = Number(c.value)
    return c.op === '=' ? a === b : c.op === '!=' ? a !== b : c.op === '<' ? a < b : c.op === '<=' ? a <= b : c.op === '>' ? a > b : a >= b
  }
  return c.op === '=' ? raw === c.value : c.op === '!=' ? raw !== c.value : false
}

export type Result = { outcome: string | null; row: number | null; trace: { row: number; failed: string[] }[] }

export function evaluate(t: Table, sample: Record<string, string>): Result {
  const trace: Result['trace'] = []
  for (const [n, r] of t.rows.entries()) {
    const failed = r.when.filter((c) => !holds(t, c, sample)).map((c) => conditionText(t, c))
    trace.push({ row: n, failed })
    if (failed.length === 0) return { outcome: r.outcome, row: n, trace }
  }
  return { outcome: t.default, row: null, trace }
}

/** Every combination of the inputs' sample values. */
export function samples(t: Table): Record<string, string>[] {
  const domains = t.inputs.map((i) => (i.type === 'code' ? (i.values ?? []) : i.type === 'boolean' ? ['true', 'false'] : (i.samples ?? []).map(String)))
  let out: Record<string, string>[] = [{}]
  for (const [n, input] of t.inputs.entries()) out = out.flatMap((s) => (domains[n] ?? []).map((v) => ({ ...s, [input.id]: v })))
  return out
}

export function coverage(t: Table): { gaps: Record<string, string>[]; neverFires: number[] } {
  const all = samples(t).map((s) => ({ s, r: evaluate(t, s) }))
  const gaps = all.filter((x) => x.r.outcome === null).map((x) => x.s)
  const fired = new Set(all.map((x) => x.r.row).filter((r): r is number => r !== null))
  return { gaps, neverFires: t.rows.map((_, n) => n).filter((n) => !fired.has(n)) }
}

export const describe = (t: Table, s: Record<string, string>) => t.inputs.map((i) => `${i.label.split(' ')[0]} ${s[i.id] ?? '?'}`).join(', ')

// --- Rule trees (variant D): each rule's condition is a tree of groups joined by all, any or not.
// A tree compiles to one FHIRPath expression (`and`, `or`, `.not()`), so the engine's rows are
// unchanged: a row's `when` holds that one expression.

export type Join = 'all' | 'any' | 'not'
export type Node = { kind: 'cond'; c: Condition } | { kind: 'group'; join: Join; children: Node[] }
export type TreeRow = { tree: Node; outcome: string }

/** Whether a node holds; `not` means none of its children hold. An empty group holds. */
export function holdsNode(t: Table, n: Node, sample: Record<string, string>): boolean {
  if (n.kind === 'cond') return holds(t, n.c, sample)
  const results = n.children.map((child) => holdsNode(t, child, sample))
  if (n.join === 'all') return results.every(Boolean)
  if (n.join === 'any') return results.length === 0 || results.some(Boolean)
  return !results.some(Boolean)
}

export function nodeText(t: Table, n: Node): string {
  if (n.kind === 'cond') return conditionText(t, n.c)
  if (n.children.length === 0) return 'true'
  const parts = n.children.map((child) => (child.kind === 'group' && child.join !== 'not' && child.children.length > 1 ? `(${nodeText(t, child)})` : nodeText(t, child)))
  if (n.join === 'all') return parts.join(' and ')
  if (n.join === 'any') return parts.join(' or ')
  return `(${parts.join(' or ')}).not()`
}

const cond = (input: string, op: Op, value: string): Node => ({ kind: 'cond', c: { input, op, value } })
const group = (join: Join, ...children: Node[]): Node => ({ kind: 'group', join, children })

export const TREE_ROWS: TreeRow[] = [
  { tree: group('all', cond('category', '=', 'emergency')), outcome: 'bypass' },
  { tree: group('any', group('all', cond('age', '<', '5'), cond('danger', '=', 'true')), group('all', cond('pregnant', '=', 'true'), cond('category', '=', 'urgent'))), outcome: 'bypass' },
  { tree: group('all', cond('category', '=', 'urgent'), group('not', cond('pregnant', '=', 'true'))), outcome: 'normal' },
  { tree: group('all', cond('category', '=', 'routine')), outcome: 'normal' },
]

export function evaluateTree(t: Table, rows: TreeRow[], sample: Record<string, string>): { outcome: string | null; row: number | null } {
  for (const [n, r] of rows.entries()) if (holdsNode(t, r.tree, sample)) return { outcome: r.outcome, row: n }
  return { outcome: t.default, row: null }
}

export function coverageTree(t: Table, rows: TreeRow[]): { gaps: Record<string, string>[]; neverFires: number[] } {
  const all = samples(t).map((s) => evaluateTree(t, rows, s))
  const fired = new Set(all.map((r) => r.row).filter((r): r is number => r !== null))
  return { gaps: samples(t).filter((_, n) => all[n]?.outcome === null), neverFires: rows.map((_, n) => n).filter((n) => !fired.has(n)) }
}
