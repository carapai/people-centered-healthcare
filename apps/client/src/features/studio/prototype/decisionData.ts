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

function holds(t: Table, c: Condition, sample: Record<string, string>): boolean {
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
