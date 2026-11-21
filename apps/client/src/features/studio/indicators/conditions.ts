// An indicator's criteria as a condition tree (studio.md § Indicators, *Criteria*; #550), as pure
// functions for the indicator editor (#537).
//
// The tree is the decision tables' (`workflows/decisions.ts`): groups joined by all, any or not
// over conditions, saved as one FHIRPath expression and parsed back when it opens. An indicator's
// tree also holds:
//
// - **+ date** on a declared date (any FHIRPath giving one): *is in the reporting period*, *is on*,
//   *before* or *after* the period's start, its end or a fixed date, *is between* two of those,
//   *is within* N days, weeks or months before or after one, *is in the previous* N periods (of the
//   Report's period type), *is empty* and *is not empty*. They compile against `%periodStart` and
//   `%periodEnd` (`YYYY-MM-DD`, compared as dates), and the previous periods against
//   `%previousPeriods.start` (the starts of the periods before, the nearest first), which
//   evaluation binds (`health-analytics-app`).
// - **+ in another indicator**: the patient is in another indicator's numerator or denominator,
//   pinned to its `url|version`: `'<url>|<version>' in %indicators.numerator`, which evaluation
//   binds and Validation reads back (`health_analytics::membership`).
//
// Each compiles to one text that `read` recognises; anything else stays FHIRPath text.

import { compile, parse, type Atoms, type Group, type Node } from '../workflows/decisions'

/** A date relative to the reporting period, or a fixed one (`YYYY-MM-DD`). */
export type Anchor = { at: 'periodStart' } | { at: 'periodEnd' } | { at: 'date'; date: string }

export const DATE_UNITS = ['days', 'weeks', 'months'] as const
export type DateUnit = (typeof DATE_UNITS)[number]

export const DATE_OPS = ['inPeriod', 'on', 'before', 'after', 'between', 'within', 'previous', 'empty', 'notEmpty'] as const
export type DateOp = (typeof DATE_OPS)[number]

/** A condition on a declared date. `input` is the FHIRPath giving the date. */
export type DateCondition =
  | { kind: 'date'; input: string; op: 'inPeriod' | 'empty' | 'notEmpty' }
  | { kind: 'date'; input: string; op: 'on' | 'before' | 'after'; anchor: Anchor }
  | { kind: 'date'; input: string; op: 'between'; from: Anchor; to: Anchor }
  | { kind: 'date'; input: string; op: 'within'; amount: number; unit: DateUnit; direction: 'before' | 'after'; anchor: Anchor }
  | { kind: 'date'; input: string; op: 'previous'; periods: number }

export const SIDES = ['numerator', 'denominator'] as const
export type Side = (typeof SIDES)[number]

/** The patient is in another indicator's numerator or denominator. */
export interface Membership {
  kind: 'member'
  /** The other indicator, `url|version`. */
  measure: string
  side: Side
}

export type Atom = DateCondition | Membership
export type CriteriaNode = Node<Atom>
export type Criteria = Group<Atom>

/** How many periods back *is in the previous* can reach (`health_analytics::PREVIOUS_PERIODS`). */
export const PREVIOUS_PERIODS = 60

// ---------------------------------------------------------------------------------------------
// Writing

const PERIOD_START = '%periodStart.toDate()'
const PERIOD_END = '%periodEnd.toDate()'

const anchorText = (anchor: Anchor): string => (anchor.at === 'periodStart' ? PERIOD_START : anchor.at === 'periodEnd' ? PERIOD_END : `@${anchor.date}`)

/** A FHIRPath calendar duration: `4 weeks`, `1 month`. */
const duration = (amount: number, unit: DateUnit): string => `${amount} ${amount === 1 ? unit.slice(0, -1) : unit}`

function compileDate(condition: DateCondition): string {
  const d = `${condition.input}.toDate()`
  switch (condition.op) {
    case 'inPeriod':
      return `(${d} >= ${PERIOD_START} and ${d} <= ${PERIOD_END})`
    case 'empty':
      return `${d}.empty()`
    case 'notEmpty':
      return `${d}.exists()`
    case 'on':
      return `${d} = ${anchorText(condition.anchor)}`
    case 'before':
      return `${d} < ${anchorText(condition.anchor)}`
    case 'after':
      return `${d} > ${anchorText(condition.anchor)}`
    case 'between':
      return `(${d} >= ${anchorText(condition.from)} and ${d} <= ${anchorText(condition.to)})`
    case 'within': {
      const anchor = anchorText(condition.anchor)
      const span = duration(condition.amount, condition.unit)
      return condition.direction === 'before' ? `(${d} >= ${anchor} - ${span} and ${d} < ${anchor})` : `(${d} > ${anchor} and ${d} <= ${anchor} + ${span})`
    }
    case 'previous':
      return `(${d} >= %previousPeriods.start[${condition.periods - 1}].toDate() and ${d} < ${PERIOD_START})`
  }
}

const compileAtom = (atom: Atom): string => (atom.kind === 'member' ? `'${atom.measure}' in %indicators.${atom.side}` : compileDate(atom))

// ---------------------------------------------------------------------------------------------
// Reading back: only what `compileAtom` writes.

/** Where the FHIRPath term starting at `text[0]` ends: at the first space or unmatched `)` outside
 * parentheses and quotes. */
function termEnd(text: string): number {
  let depth = 0
  for (let at = 0; at < text.length; at++) {
    const char = text[at]
    if (char === "'") {
      for (at++; at < text.length && text[at] !== "'"; at++) if (text[at] === '\\') at++
    } else if (char === '(' || char === '[') depth++
    else if (char === ')' || char === ']') {
      if (depth === 0) return at
      depth--
    } else if (depth === 0 && /\s/.test(char ?? '')) return at
  }
  return text.length
}

/** The index of the `)` closing the `(` at `text[0]`, or -1. */
function closing(text: string): number {
  let depth = 0
  for (let at = 0; at < text.length; at++) {
    const char = text[at]
    if (char === "'") {
      for (at++; at < text.length && text[at] !== "'"; at++) if (text[at] === '\\') at++
    } else if (char === '(') depth++
    else if (char === ')') {
      depth--
      if (depth === 0) return at
    }
  }
  return -1
}

const DATE = /^\d{4}-\d{2}-\d{2}$/

/** An anchor's text, with what it adds or takes away. */
interface Bound {
  anchor: Anchor | { at: 'previous'; index: number }
  offset: { sign: '+' | '-'; amount: number; unit: DateUnit } | null
}

function readAnchor(text: string): Bound | null {
  const match = /^(%periodStart\.toDate\(\)|%periodEnd\.toDate\(\)|@(\d{4}-\d{2}-\d{2})|%previousPeriods\.start\[(\d+)\]\.toDate\(\))(?: ([+-]) (\d+) (day|days|week|weeks|month|months))?$/.exec(text)
  if (match === null) return null
  const [, base, date, index, sign, amount, unit] = match
  const anchor: Bound['anchor'] =
    base === PERIOD_START
      ? { at: 'periodStart' }
      : base === PERIOD_END
        ? { at: 'periodEnd' }
        : date !== undefined && DATE.test(date)
          ? { at: 'date', date }
          : { at: 'previous', index: Number(index) }
  const offset = sign === undefined ? null : { sign: sign as '+' | '-', amount: Number(amount), unit: (unit?.endsWith('s') ? unit : `${unit}s`) as DateUnit }
  return { anchor, offset }
}

/** `<input>.toDate() <op> <anchor>`: the input, the operator and the anchor. */
function readComparison(text: string): { input: string; op: string; side: Bound } | null {
  const end = termEnd(text)
  const term = text.slice(0, end)
  if (!term.endsWith('.toDate()')) return null
  const match = /^ (>=|<=|=|<|>) (.+)$/.exec(text.slice(end))
  const side = match?.[2] === undefined ? null : readAnchor(match[2])
  if (match?.[1] === undefined || side === null) return null
  return { input: term.slice(0, -'.toDate()'.length), op: match[1], side }
}

const plain = (side: Bound): Anchor | null => (side.offset === null && side.anchor.at !== 'previous' ? side.anchor : null)
const same = (a: Anchor, b: Anchor) => a.at === b.at && (a.at !== 'date' || (b.at === 'date' && a.date === b.date))

/** The date condition in `inner`, the text between a pair's parentheses. */
function readPair(inner: string): DateCondition | null {
  // The second comparison starts at the ` and ` after the first's date.
  const at = inner.indexOf(' and ', termEnd(inner))
  if (at < 0) return null
  const lower = readComparison(inner.slice(0, at))
  const upper = readComparison(inner.slice(at + 5))
  if (lower === null || upper === null || lower.input !== upper.input) return null
  const input = lower.input
  const [lo, hi] = [plain(lower.side), plain(upper.side)]
  if (lower.op === '>=' && upper.op === '<=' && lo !== null && hi !== null) {
    if (lo.at === 'periodStart' && hi.at === 'periodEnd') return { kind: 'date', input, op: 'inPeriod' }
    return { kind: 'date', input, op: 'between', from: lo, to: hi }
  }
  const offset = lower.side.offset ?? upper.side.offset
  if (lower.op === '>=' && upper.op === '<' && hi !== null && lower.side.anchor.at === 'previous' && lower.side.offset === null && hi.at === 'periodStart') {
    return { kind: 'date', input, op: 'previous', periods: lower.side.anchor.index + 1 }
  }
  if (lower.op === '>=' && upper.op === '<' && hi !== null && offset?.sign === '-' && upper.side.offset === null) {
    const base = lower.side.anchor
    if (base.at !== 'previous' && same(base, hi)) return { kind: 'date', input, op: 'within', amount: offset.amount, unit: offset.unit, direction: 'before', anchor: hi }
  }
  if (lower.op === '>' && upper.op === '<=' && lo !== null && offset?.sign === '+' && lower.side.offset === null) {
    const base = upper.side.anchor
    if (base.at !== 'previous' && same(base, lo)) return { kind: 'date', input, op: 'within', amount: offset.amount, unit: offset.unit, direction: 'after', anchor: lo }
  }
  return null
}

const OPS: Record<string, 'on' | 'before' | 'after'> = { '=': 'on', '<': 'before', '>': 'after' }

/** The condition at the start of `rest`, with the length of its text. */
function readAtom(rest: string): { atom: Atom; length: number } | null {
  const member = /^'([^'\\]*)' in %indicators\.(numerator|denominator)(?![\w(])/.exec(rest)
  if (member !== null) return { atom: { kind: 'member', measure: member[1] ?? '', side: member[2] as Side }, length: member[0].length }
  if (rest.startsWith('(')) {
    const close = closing(rest)
    if (close < 0) return null
    const pair = readPair(rest.slice(1, close))
    return pair === null ? null : { atom: pair, length: close + 1 }
  }
  const end = termEnd(rest)
  const term = rest.slice(0, end)
  for (const [suffix, op] of [
    ['.toDate().empty()', 'empty'],
    ['.toDate().exists()', 'notEmpty'],
  ] as const) {
    if (term.endsWith(suffix) && term.length > suffix.length) return { atom: { kind: 'date', input: term.slice(0, -suffix.length), op }, length: end }
  }
  if (!term.endsWith('.toDate()')) return null
  // `<input>.toDate() <op> <anchor>` up to the next space or `)` after the anchor.
  const match = /^ (=|<|>) (%periodStart\.toDate\(\)|%periodEnd\.toDate\(\)|@\d{4}-\d{2}-\d{2})(?=$|\s|\))/.exec(rest.slice(end))
  const anchor = match?.[2] === undefined ? null : readAnchor(match[2])
  const op = match?.[1] === undefined ? undefined : OPS[match[1]]
  if (match === null || anchor === null || op === undefined || anchor.offset !== null || anchor.anchor.at === 'previous') return null
  return { atom: { kind: 'date', input: term.slice(0, -'.toDate()'.length), op, anchor: anchor.anchor }, length: end + match[0].length }
}

export const ATOMS: Atoms<Atom> = { compile: compileAtom, read: readAtom }

/** The criteria as one FHIRPath expression. */
export const compileCriteria = (node: CriteriaNode): string => compile(node, ATOMS)

/** The tree an indicator's criteria hold: what `compileCriteria` wrote, or the text kept as
 * FHIRPath. */
export const parseCriteria = (expression: string): Criteria => parse(expression, ATOMS)

/** The other indicators a tree refers to, `url|version`, as Validation reads them. */
export function referencesOf(node: CriteriaNode): Membership[] {
  if ((node as { kind: string }).kind === 'member') return [node as Membership]
  if ((node as { kind: string }).kind !== 'group') return []
  return (node as Criteria).children.flatMap(referencesOf)
}
