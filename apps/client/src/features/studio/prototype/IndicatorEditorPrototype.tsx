// PROTOTYPE (#534), throwaway: the indicator editor on /prototype/indicator-editor. Round 2 of
// variant A (definition left, Try beside it), after the first round's pick: an indicator is
// numerator / denominator, the denominator can be 1 (a count), there is no separate exclusion
// population (a "none of these" group does that), and each side starts from everyone in the
// period, this indicator's denominator, or another indicator's numerator or denominator, then
// adds conditions. Round 3 drops "starts from": an "is in another indicator" condition in the
// tree does the same and more. Round 1's variants B and C are in this branch's history.
import { css, cx } from '@health/design-system/css'
import { useState } from 'react'

import {
  DISAGGREGATIONS,
  HMIS_MAPPING,
  INITIAL,
  INPUTS,
  PERIOD_DATES,
  PERIODS,
  baseLabel,
  baseOf,
  compiled,
  evaluate,
  holds,
  display,
  refOptions,
  stripDenominator,
  type Base,
  type Ctx,
  type Indicator,
  type Join,
  type Node,
  type Op,
  type Part,
  type Patient,
  type SavedTest,
  type Side,
} from './indicatorData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const small = cx(ghost, css({ minHeight: 'auto', py: '0', px: '2', fontSize: 'xs' }))
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs', color: 'fg.muted', wordBreak: 'break-word' })
const ok = css({ color: 'success.text' })
const bad = css({ color: 'danger.text' })
const OPS: Op[] = ['=', '!=', '<', '<=', '>', '>=']
const JOINS: Record<Join, string> = { all: 'all of these', any: 'any of these', not: 'none of these' }

function TreeEditor({ node, onChange, ind, side, patient, ctx, depth = 0 }: { node: Node; onChange: (n: Node) => void; ind: Indicator; side: Side; patient?: Patient | undefined; ctx?: Ctx | undefined; depth?: number }) {
  const mark = patient && ctx ? (holds(node, patient, ctx) ? <span className={ok}>✓ </span> : <span className={bad}>✗ </span>) : null
  if (node.kind === 'member' && node.ref.kind === 'denominator') {
    return (
      <div className={css({ display: 'flex', gap: '1', alignItems: 'center', fontSize: 'sm' })}>
        {mark}
        is in this indicator's denominator
      </div>
    )
  }
  if (node.kind === 'member') {
    return (
      <div className={css({ display: 'flex', gap: '1', alignItems: 'center', flexWrap: 'wrap' })}>
        {mark}
        <span className={css({ fontSize: 'sm' })}>is in</span>
        <select className={input} value={enc(node.ref)} aria-label="Indicator population" onChange={(e) => onChange({ kind: 'member', ref: dec(e.target.value) })}>
          {refOptions().map((b) => (
            <option key={enc(b)} value={enc(b)}>
              {baseLabel(b)}
            </option>
          ))}
        </select>
      </div>
    )
  }
  if (node.kind === 'cond') {
    const inp = INPUTS.find((i) => i.id === node.input)
    return (
      <div className={css({ display: 'flex', gap: '1', alignItems: 'center', flexWrap: 'wrap' })}>
        {mark}
        <select className={input} value={node.input} aria-label="Input" onChange={(e) => onChange({ ...node, input: e.target.value })}>
          {INPUTS.map((i) => (
            <option key={i.id} value={i.id}>
              {i.label}
            </option>
          ))}
        </select>
        <select className={input} value={node.op} aria-label="Operator" onChange={(e) => onChange({ ...node, op: e.target.value as Op })}>
          {OPS.map((o) => (
            <option key={o}>{o}</option>
          ))}
        </select>
        {inp?.values ? (
          <select className={input} value={node.value} aria-label="Value" onChange={(e) => onChange({ ...node, value: e.target.value })}>
            {inp.values.map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        ) : inp?.type === 'boolean' ? (
          <select className={input} value={node.value} aria-label="Value" onChange={(e) => onChange({ ...node, value: e.target.value })}>
            <option>true</option>
            <option>false</option>
          </select>
        ) : (
          <input className={cx(input, css({ width: '5rem' }))} value={node.value} aria-label="Value" onChange={(e) => onChange({ ...node, value: e.target.value })} />
        )}
      </div>
    )
  }
  const set = (i: number, n: Node | null) => onChange({ ...node, children: n ? node.children.map((ch, j) => (j === i ? n : ch)) : node.children.filter((_, j) => j !== i) })
  return (
    <div className={css({ borderLeft: depth ? '2px solid' : 'none', borderColor: 'border', pl: depth ? '3' : '0', display: 'flex', flexDirection: 'column', gap: '1' })}>
      <div className={css({ display: 'flex', gap: '1', alignItems: 'center' })}>
        {mark}
        <select className={input} value={node.join} aria-label="Join" onChange={(e) => onChange({ ...node, join: e.target.value as Join })}>
          {(Object.keys(JOINS) as Join[]).map((j) => (
            <option key={j} value={j}>
              {JOINS[j]}
            </option>
          ))}
        </select>
        <button type="button" className={small} onClick={() => onChange({ ...node, children: [...node.children, { kind: 'cond', input: 'age', op: '>=', value: '15' }] })}>
          + condition
        </button>
        <button type="button" className={small} onClick={() => onChange({ ...node, children: [...node.children, { kind: 'group', join: 'any', children: [] }] })}>
          + group
        </button>
        <button type="button" className={small} onClick={() => onChange({ ...node, children: [...node.children, { kind: 'member', ref: refOptions()[0] ?? { kind: 'period' } }] })}>
          + in another indicator
        </button>
        {side === 'numerator' && ind.denominator !== 'one' ? (
          <button type="button" className={small} onClick={() => onChange({ ...node, children: [...node.children, { kind: 'member', ref: { kind: 'denominator' } }] })}>
            + in the denominator
          </button>
        ) : null}
      </div>
      {node.children.length === 0 ? <span className={muted}>Everyone from the step before.</span> : null}
      {node.children.map((ch, i) => (
        <div key={i} className={css({ display: 'flex', gap: '1', alignItems: 'flex-start' })}>
          <div className={css({ flex: '1' })}>
            <TreeEditor node={ch} onChange={(n) => set(i, n)} ind={ind} side={side} patient={patient} ctx={ctx} depth={depth + 1} />
          </div>
          <button type="button" className={small} aria-label="Remove" onClick={() => set(i, null)}>
            ×
          </button>
        </div>
      ))}
    </div>
  )
}

const enc = (b: Base) => (b.kind === 'indicator' ? `indicator:${b.id}:${b.side}` : b.kind)
const dec = (s: string): Base => {
  const [k, id, side] = s.split(':')
  return k === 'indicator' && id && side ? { kind: 'indicator', id, side: side as Side } : k === 'denominator' ? { kind: 'denominator' } : { kind: 'period' }
}

function SideCard({ ind, side, setInd, count }: { ind: Indicator; side: Side; setInd: (i: Indicator) => void; count: number | null }) {
  const part = side === 'numerator' ? ind.numerator : ind.denominator
  const set = (p: Part | 'one') => {
    if (side === 'numerator' && p !== 'one') return setInd({ ...ind, numerator: p })
    // Turning the denominator into 1 moves a numerator that started from it back to the period.
    // Turning the denominator into 1 removes the numerator's "is in the denominator" conditions.
    return setInd({ ...ind, denominator: p, numerator: p === 'one' ? { ...ind.numerator, tree: stripDenominator(ind.numerator.tree) } : ind.numerator })
  }
  return (
    <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
      <div className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' })}>
        <h2 className={h2}>{side === 'numerator' ? 'Numerator' : 'Denominator'}</h2>
        {count === null ? null : <span className={muted}>{count} in the period tried</span>}
      </div>
      {side === 'denominator' ? (
        <div role="radiogroup" aria-label="Denominator" className={css({ display: 'flex', gap: '3', fontSize: 'sm' })}>
          <label>
            <input type="radio" checked={part === 'one'} onChange={() => set('one')} /> 1: the indicator is a count of the numerator
          </label>
          <label>
            <input type="radio" checked={part !== 'one'} onChange={() => set({ base: { kind: 'period' }, tree: { kind: 'group', join: 'all', children: [] } })} /> Patients, so the indicator is a percentage
          </label>
        </div>
      ) : null}
      {part === 'one' ? null : (
        <>
          <span className={css({ fontSize: 'sm' })}>Patients with care in the period who match</span>
          <TreeEditor node={part.tree} onChange={(tree) => set({ ...part, tree })} ind={ind} side={side} />
          <code className={mono}>{compiled(ind, side)}</code>
        </>
      )}
    </section>
  )
}

function Settings({ ind, setInd }: { ind: Indicator; setInd: (i: Indicator) => void }) {
  return (
    <div className={css({ display: 'flex', flexWrap: 'wrap', gap: '4', fontSize: 'sm' })}>
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        Period date: what puts care in a period (required)
        <select className={input} value={ind.periodDate} onChange={(e) => setInd({ ...ind, periodDate: e.target.value as Indicator['periodDate'] })}>
          {PERIOD_DATES.map((d) => (
            <option key={d.id} value={d.id}>
              {d.label}
            </option>
          ))}
        </select>
      </label>
      <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        <legend>Disaggregations</legend>
        {DISAGGREGATIONS.map((d) => (
          <label key={d.id}>
            <input type="checkbox" checked={ind.disaggregations.includes(d.id)} onChange={(e) => setInd({ ...ind, disaggregations: e.target.checked ? [...ind.disaggregations, d.id] : ind.disaggregations.filter((x) => x !== d.id) })} /> {d.label}
          </label>
        ))}
      </fieldset>
    </div>
  )
}

/** Patients on one side, each opening to that side's conditions marked for them. */
function Patients({ ind, side, list, setInd, period }: { ind: Indicator; side: Side; list: Patient[]; setInd: (i: Indicator) => void; period: string }) {
  const ctx: Ctx = { ind, period, seen: [`${ind.id}.${side}`] }
  const [open, setOpen] = useState<string | null>(null)
  const part = side === 'numerator' ? ind.numerator : ind.denominator
  return (
    <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
      {list.length === 0 ? <li className={muted}>Nobody.</li> : null}
      {list.map((p) => (
        <li key={p.id}>
          <button type="button" className={css({ cursor: 'pointer', textAlign: 'left' })} onClick={() => setOpen(open === p.id ? null : p.id)}>
            {open === p.id ? '▾' : '▸'} {p.name}, {p.age} · {p.encounterType} · GA {p.gestationWeeks}w · visit {p.start}
            {p.recorded !== p.start ? <span className={muted}> (entered {p.recorded})</span> : null}
          </button>
          {open === p.id && part !== 'one' ? (
            <div className={cx(box, css({ p: '2', mt: '1', bg: 'bg.subtle' }))}>
                            <TreeEditor node={part.tree} onChange={(tree) => setInd(side === 'numerator' ? { ...ind, numerator: { ...part, tree } } : { ...ind, denominator: { ...part, tree } })} ind={ind} side={side} patient={p} ctx={ctx} />
            </div>
          ) : null}
        </li>
      ))}
    </ul>
  )
}

function Dropped({ ind, side, period }: { ind: Indicator; side: Side; period: string }) {
  const part = side === 'numerator' ? ind.numerator : ind.denominator
  if (part === 'one') return null
  const from = baseOf(ind, part.base, period)
  const out = from.filter((p) => !holds(part.tree, p, { ind, period, seen: [`${ind.id}.${side}`] }))
  return (
    <p className={muted}>
      {from.length} with care in the period; {out.length} don't match
      {out.length ? `: ${out.map((p) => p.name).join(', ')}` : ''}.
    </p>
  )
}

export function IndicatorEditorPrototype() {
  const [ind, setInd] = useState<Indicator>(INITIAL)
  const [period, setPeriod] = useState(PERIODS[0] ?? '2026-03')
  const [drill, setDrill] = useState<Side>('numerator')
  const [tests, setTests] = useState<SavedTest[]>([])
  const r = evaluate(ind, period)
  const sides: Side[] = r.denominator === null ? ['numerator'] : ['numerator', 'denominator']
  const bands = DISAGGREGATIONS.filter((d) => ind.disaggregations.includes(d.id)).flatMap((d) => d.bands.map((b) => ({ ...b, key: `${d.label}: ${b.label}` })))
  const expectNow = { numerator: r.numerator.length, denominator: r.denominator === null ? ('one' as const) : r.denominator.length }
  const pass = (t: SavedTest) => {
    const x = evaluate(ind, t.period)
    return t.expect.numerator === x.numerator.length && t.expect.denominator === (x.denominator === null ? 'one' : x.denominator.length)
  }
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', pb: '8' })}>
      <header>
        <p className={muted}>← ug.moh.anc 2.4.0, draft · Indicators</p>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{ind.title}</h1>
        <p className={mono}>https://health.go.ug/fhir/Measure/{ind.id}</p>
      </header>
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 26rem' }, gap: '4', alignItems: 'start' })}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <section className={cx(box, css({ p: '3' }))}>
            <Settings ind={ind} setInd={setInd} />
            <p className={cx(muted, css({ fontSize: 'xs', mt: '2' }))}>Mapped to {HMIS_MAPPING} (read-only)</p>
          </section>
          <SideCard ind={ind} side="numerator" setInd={setInd} count={r.numerator.length} />
          <SideCard ind={ind} side="denominator" setInd={setInd} count={r.denominator === null ? null : r.denominator.length} />
        </div>
        <aside className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '3', position: { lg: 'sticky' }, top: '4' }))}>
          <h2 className={h2}>Try over test patients</h2>
          <div role="radiogroup" aria-label="Period" className={css({ display: 'flex', gap: '1' })}>
            {PERIODS.map((p) => (
              <button key={p} type="button" role="radio" aria-checked={period === p} className={cx(small, period === p ? css({ bg: 'bg.selected' }) : undefined)} onClick={() => setPeriod(p)}>
                {p}
              </button>
            ))}
          </div>
          <p className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
            {display(r)} {r.denominator === null ? <span className={muted}>(a count)</span> : <span className={muted}>({r.numerator.length} of {r.denominator.length})</span>}
          </p>
          <table className={css({ fontSize: 'sm', '& td, & th': { px: '1', textAlign: 'right' }, '& th:first-child, & td:first-child': { textAlign: 'left' } })}>
            <thead>
              <tr>
                <th />
                {sides.map((s) => (
                  <th key={s}>
                    <button type="button" className={cx(css({ cursor: 'pointer' }), drill === s ? css({ textDecoration: 'underline' }) : undefined)} onClick={() => setDrill(s)}>
                      {s === 'numerator' ? 'Numerator' : 'Denominator'}
                    </button>
                  </th>
                ))}
                {r.denominator === null ? null : <th>%</th>}
              </tr>
            </thead>
            <tbody>
              {[{ key: 'All', test: () => true }, ...bands].map((b) => {
                const n = r.numerator.filter(b.test).length
                const d = r.denominator?.filter(b.test).length
                return (
                  <tr key={b.key}>
                    <td className={b.key === 'All' ? undefined : muted}>{b.key}</td>
                    <td>{n}</td>
                    {d === undefined ? null : <td>{d}</td>}
                    {d === undefined ? null : <td>{d ? `${Math.round((100 * n) / d)}%` : '–'}</td>}
                  </tr>
                )
              })}
            </tbody>
          </table>
          <h3 className={css({ fontWeight: 'semibold' })}>{drill === 'numerator' ? 'Numerator' : 'Denominator'}: who and why</h3>
          <Dropped ind={ind} side={sides.includes(drill) ? drill : 'numerator'} period={period} />
          <Patients ind={ind} side={sides.includes(drill) ? drill : 'numerator'} list={sides.includes(drill) && drill === 'denominator' ? (r.denominator ?? []) : r.numerator} setInd={setInd} period={period} />
          <button type="button" className={ghost} onClick={() => setTests([...tests.filter((t) => t.period !== period), { period, expect: expectNow }])}>
            Save as test ({period})
          </button>
          {tests.length === 0 ? <span className={muted}>No saved tests. Submission runs them and fails if the counts change.</span> : null}
          {tests.map((t) => (
            <div key={t.period} className={cx(css({ fontSize: 'sm' }), pass(t) ? ok : bad)}>
              {pass(t) ? '✓' : '✗ would fail submission:'} {t.period} expects numerator {t.expect.numerator}, denominator {t.expect.denominator === 'one' ? '1' : t.expect.denominator}
            </div>
          ))}
        </aside>
      </div>
    </section>
  )
}
