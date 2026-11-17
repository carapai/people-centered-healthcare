// PROTOTYPE (#534), throwaway: three layouts for the indicator editor on /prototype/indicator-editor,
// switched with ?variant=A|B|C and the floating bar (← / → keys). Behaviour is fixed by
// studio.md § Indicators (#530); only the layout is in question.
// A: the definition on the left, Try always beside it on the right.
// B: results first: a live grid of populations × disaggregation bands; click a cell to edit
//    that population and see its patients.
// C: a funnel: in period → initial → denominator → exclusions → numerator; click a step to edit
//    it and see who it let through and who it dropped, and why.
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import {
  DISAGGREGATIONS,
  HMIS_MAPPING,
  INITIAL,
  INPUTS,
  PERIOD_DATES,
  PERIODS,
  condText,
  evaluate,
  fhirpath,
  holds,
  pct,
  type Indicator,
  type Join,
  type Kind,
  type Node,
  type Op,
  type Patient,
  type PopKey,
  type Result,
  type SavedTest,
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

// --- Shared state ------------------------------------------------------------------------------

function useIndicator() {
  const [ind, setInd] = useState<Indicator>(INITIAL)
  const [period, setPeriod] = useState(PERIODS[0] ?? '2026-03')
  const [tests, setTests] = useState<SavedTest[]>([])
  const r = evaluate(ind, period)
  const setTree = (k: PopKey, tree: Node) => setInd({ ...ind, populations: ind.populations.map((p) => (p.key === k ? { ...p, tree } : p)) })
  const pops = ind.kind === 'count' ? ind.populations.filter((p) => p.key === 'initial') : ind.populations
  const saveTest = () => setTests([...tests.filter((t) => t.period !== period), { period, expect: counts(r) }])
  return { ind, setInd, period, setPeriod, tests, saveTest, r, setTree, pops }
}
type M = ReturnType<typeof useIndicator>
const counts = (r: Result) => ({ initial: r.initial.length, denominator: r.denominator.length, exclusion: r.exclusion.length, numerator: r.numerator.length })
const popLabel = (ind: Indicator, k: PopKey) => (ind.kind === 'count' && k === 'initial' ? 'Counted' : (ind.populations.find((p) => p.key === k)?.label ?? k))

// --- Shared pieces -----------------------------------------------------------------------------

function TreeEditor({ node, onChange, patient, depth = 0 }: { node: Node; onChange: (n: Node) => void; patient?: Patient | undefined; depth?: number }) {
  const mark = patient ? (holds(node, patient) ? <span className={ok}>✓ </span> : <span className={bad}>✗ </span>) : null
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
      </div>
      {node.children.length === 0 ? <span className={muted}>Everyone from the step before.</span> : null}
      {node.children.map((ch, i) => (
        <div key={i} className={css({ display: 'flex', gap: '1', alignItems: 'flex-start' })}>
          <div className={css({ flex: '1' })}>
            <TreeEditor node={ch} onChange={(n) => set(i, n)} patient={patient} depth={depth + 1} />
          </div>
          <button type="button" className={small} aria-label="Remove" onClick={() => set(i, null)}>
            ×
          </button>
        </div>
      ))}
    </div>
  )
}

function Settings({ m, inline = false }: { m: M; inline?: boolean }) {
  const { ind, setInd } = m
  return (
    <div className={css({ display: 'flex', flexDirection: inline ? 'row' : 'column', flexWrap: 'wrap', gap: '3', fontSize: 'sm' })}>
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        Kind
        <select className={input} value={ind.kind} onChange={(e) => setInd({ ...ind, kind: e.target.value as Kind })}>
          <option value="proportion">Proportion (numerator / denominator)</option>
          <option value="count">Count (one population)</option>
        </select>
      </label>
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

const Mapping = () => <p className={cx(muted, css({ fontSize: 'xs' }))}>Mapped to {HMIS_MAPPING} (read-only)</p>

function PeriodPicker({ m }: { m: M }) {
  return (
    <div role="radiogroup" aria-label="Period" className={css({ display: 'flex', gap: '1' })}>
      {PERIODS.map((p) => (
        <button key={p} type="button" role="radio" aria-checked={m.period === p} className={cx(small, m.period === p ? css({ bg: 'bg.selected' }) : undefined)} onClick={() => m.setPeriod(p)}>
          {p}
        </button>
      ))}
    </div>
  )
}

function Tests({ m }: { m: M }) {
  const now = counts(m.r)
  const mine = m.tests.find((t) => t.period === m.period)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
      <button type="button" className={ghost} onClick={m.saveTest}>
        Save as test ({m.period})
      </button>
      {m.tests.length === 0 ? <span className={muted}>No saved tests. Submission runs saved tests and fails if the counts change.</span> : null}
      {m.tests.map((t) => {
        const r = evaluate(m.ind, t.period)
        const c = counts(r)
        const pass = (Object.keys(t.expect) as PopKey[]).every((k) => t.expect[k] === c[k])
        return (
          <div key={t.period} className={pass ? ok : bad}>
            {pass ? '✓' : '✗'} {t.period}: expects {Object.entries(t.expect).map(([k, v]) => `${k} ${v}`).join(', ')}
            {pass ? '' : ` (now ${Object.entries(c).map(([k, v]) => `${k} ${v}`).join(', ')})`}
          </div>
        )
      })}
      {mine && (Object.keys(mine.expect) as PopKey[]).some((k) => mine.expect[k] !== now[k]) ? <span className={bad}>This edit would fail submission.</span> : null}
    </div>
  )
}

/** Patients in a population, each opening to the condition tree marked for them. */
function Patients({ m, list, pop }: { m: M; list: Patient[]; pop: PopKey }) {
  const [open, setOpen] = useState<string | null>(null)
  const tree = m.ind.populations.find((p) => p.key === pop)?.tree
  return (
    <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
      {list.length === 0 ? <li className={muted}>Nobody.</li> : null}
      {list.map((p) => (
        <li key={p.id}>
          <button type="button" className={css({ cursor: 'pointer', textAlign: 'left' })} onClick={() => setOpen(open === p.id ? null : p.id)}>
            {open === p.id ? '▾' : '▸'} {p.name}, {p.age} · {p.encounterType} · GA {p.gestationWeeks}w · visit {p.start}
            {p.recorded !== p.start ? <span className={muted}> (entered {p.recorded})</span> : null}
          </button>
          {open === p.id && tree ? (
            <div className={cx(box, css({ p: '2', mt: '1', bg: 'bg.subtle' }))}>
              <TreeEditor node={tree} onChange={(n) => m.setTree(pop, n)} patient={p} />
            </div>
          ) : null}
        </li>
      ))}
    </ul>
  )
}

const bandsOf = (ind: Indicator) => DISAGGREGATIONS.filter((d) => ind.disaggregations.includes(d.id))

// --- Variant A: definition left, Try right ---------------------------------------------------------

function VariantA({ m }: { m: M }) {
  const [drill, setDrill] = useState<PopKey | null>('numerator')
  const pops: PopKey[] = m.ind.kind === 'count' ? ['initial'] : ['initial', 'denominator', 'exclusion', 'numerator']
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 26rem' }, gap: '4', alignItems: 'start' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <section className={cx(box, css({ p: '3' }))}>
          <Settings m={m} inline />
          <Mapping />
        </section>
        {m.pops.map((p) => (
          <section key={p.key} className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h2 className={h2}>{popLabel(m.ind, p.key)}</h2>
            <TreeEditor node={p.tree} onChange={(n) => m.setTree(p.key, n)} />
            <code className={mono}>{fhirpath(p.tree)}</code>
          </section>
        ))}
      </div>
      <aside className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '3', position: { lg: 'sticky' }, top: '4' }))}>
        <h2 className={h2}>Try over test patients</h2>
        <PeriodPicker m={m} />
        {m.ind.kind === 'proportion' ? (
          <p className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
            {pct(m.r)}% <span className={muted}>({m.r.numerator.length} of {m.r.denominator.length})</span>
          </p>
        ) : (
          <p className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{m.r.initial.length}</p>
        )}
        <table className={css({ fontSize: 'sm', '& td, & th': { px: '1', textAlign: 'right' }, '& th:first-child, & td:first-child': { textAlign: 'left' } })}>
          <thead>
            <tr>
              <th />
              {pops.map((k) => (
                <th key={k}>
                  <button type="button" className={cx(css({ cursor: 'pointer' }), drill === k ? css({ textDecoration: 'underline' }) : undefined)} onClick={() => setDrill(k)}>
                    {popLabel(m.ind, k)}
                  </button>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>All</td>
              {pops.map((k) => (
                <td key={k}>{m.r[k].length}</td>
              ))}
            </tr>
            {bandsOf(m.ind).flatMap((d) =>
              d.bands.map((b) => (
                <tr key={d.id + b.label}>
                  <td className={muted}>{b.label}</td>
                  {pops.map((k) => (
                    <td key={k}>{m.r[k].filter(b.test).length}</td>
                  ))}
                </tr>
              )),
            )}
          </tbody>
        </table>
        {drill ? (
          <>
            <h3 className={css({ fontWeight: 'semibold' })}>{popLabel(m.ind, drill)}: who and why</h3>
            <Patients m={m} list={m.r[drill]} pop={drill} />
          </>
        ) : null}
        <Tests m={m} />
      </aside>
    </div>
  )
}

// --- Variant B: results first ----------------------------------------------------------------------

function VariantB({ m }: { m: M }) {
  const [cell, setCell] = useState<{ pop: PopKey; band: string | null }>({ pop: 'numerator', band: null })
  const [tab, setTab] = useState<'criteria' | 'settings' | 'tests'>('criteria')
  const pops: PopKey[] = m.ind.kind === 'count' ? ['initial'] : ['initial', 'denominator', 'exclusion', 'numerator']
  const pop = pops.includes(cell.pop) ? cell.pop : 'initial'
  const bands = bandsOf(m.ind).flatMap((d) => d.bands.map((b) => ({ ...b, key: `${d.label}: ${b.label}` })))
  const bandTest = bands.find((b) => b.key === cell.band)?.test
  const list = m.r[pop].filter(bandTest ?? (() => true))
  const tree = m.ind.populations.find((p) => p.key === pop)?.tree
  const cellButton = (k: PopKey, band: string | null, n: number) => (
    <button type="button" className={cx(css({ cursor: 'pointer', width: '100%', textAlign: 'right', px: '2', py: '1', borderRadius: 'sm' }), pop === k && cell.band === band ? css({ bg: 'bg.selected', fontWeight: 'semibold' }) : undefined)} onClick={() => setCell({ pop: k, band })}>
      {n}
    </button>
  )
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <div className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '2' })}>
          <h2 className={h2}>Results over test patients</h2>
          <PeriodPicker m={m} />
        </div>
        <table className={css({ fontSize: 'sm', width: '100%', '& th': { textAlign: 'right', px: '2' }, '& th:first-child': { textAlign: 'left' } })}>
          <thead>
            <tr>
              <th />
              {pops.map((k) => (
                <th key={k}>{popLabel(m.ind, k)}</th>
              ))}
              {m.ind.kind === 'proportion' ? <th>%</th> : null}
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>All</td>
              {pops.map((k) => (
                <td key={k}>{cellButton(k, null, m.r[k].length)}</td>
              ))}
              {m.ind.kind === 'proportion' ? <td className={css({ textAlign: 'right', px: '2' })}>{pct(m.r)}%</td> : null}
            </tr>
            {bands.map((b) => (
              <tr key={b.key}>
                <td className={muted}>{b.key}</td>
                {pops.map((k) => (
                  <td key={k}>{cellButton(k, b.key, m.r[k].filter(b.test).length)}</td>
                ))}
                {m.ind.kind === 'proportion' ? <td className={cx(muted, css({ textAlign: 'right', px: '2' }))}>{m.r.denominator.filter(b.test).length ? Math.round((100 * m.r.numerator.filter(b.test).length) / m.r.denominator.filter(b.test).length) : 0}%</td> : null}
              </tr>
            ))}
          </tbody>
        </table>
        <Mapping />
      </section>
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 22rem' }, gap: '3', alignItems: 'start' })}>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
          <div role="tablist" className={css({ display: 'flex', gap: '1' })}>
            {(['criteria', 'settings', 'tests'] as const).map((t) => (
              <button key={t} type="button" role="tab" aria-selected={tab === t} className={cx(small, tab === t ? css({ bg: 'bg.selected' }) : undefined)} onClick={() => setTab(t)}>
                {t === 'criteria' ? `Criteria: ${popLabel(m.ind, pop)}` : t === 'settings' ? 'Kind, period date, disaggregations' : 'Tests'}
              </button>
            ))}
          </div>
          {tab === 'criteria' && tree ? (
            <>
              <TreeEditor node={tree} onChange={(n) => m.setTree(pop, n)} />
              <code className={mono}>{fhirpath(tree)}</code>
              <span className={muted}>Click a column in the grid to edit another population.</span>
            </>
          ) : null}
          {tab === 'settings' ? <Settings m={m} /> : null}
          {tab === 'tests' ? <Tests m={m} /> : null}
        </section>
        <aside className={cx(box, css({ p: '3' }))}>
          <h3 className={css({ fontWeight: 'semibold', mb: '2' })}>
            {popLabel(m.ind, pop)}
            {cell.band ? ` · ${cell.band}` : ''}: {list.length}
          </h3>
          <Patients m={m} list={list} pop={pop} />
        </aside>
      </div>
    </div>
  )
}

// --- Variant C: the funnel -------------------------------------------------------------------------

function VariantC({ m }: { m: M }) {
  const [step, setStep] = useState<PopKey>('numerator')
  const [band, setBand] = useState<string>('')
  const bands = bandsOf(m.ind).flatMap((d) => d.bands.map((b) => ({ ...b, key: `${d.label}: ${b.label}` })))
  const f = bands.find((b) => b.key === band)?.test ?? (() => true)
  const steps: { key: PopKey | 'period'; label: string; list: Patient[]; before: Patient[] }[] = [{ key: 'period', label: 'In period', list: m.r.inPeriod.filter(f), before: [] }]
  steps.push({ key: 'initial', label: popLabel(m.ind, 'initial'), list: m.r.initial.filter(f), before: m.r.inPeriod.filter(f) })
  if (m.ind.kind === 'proportion') {
    const denomRaw = m.r.initial.filter((p) => holds(m.ind.populations.find((x) => x.key === 'denominator')?.tree ?? { kind: 'group', join: 'all', children: [] }, p))
    steps.push({ key: 'denominator', label: 'Denominator', list: m.r.denominator.filter(f), before: m.r.initial.filter(f) })
    steps.push({ key: 'exclusion', label: '− Excluded', list: m.r.exclusion.filter(f), before: denomRaw.filter(f) })
    steps.push({ key: 'numerator', label: 'Numerator', list: m.r.numerator.filter(f), before: m.r.denominator.filter(f) })
  }
  const max = Math.max(1, ...steps.map((s) => s.list.length))
  const current = steps.find((s) => s.key === step) ?? steps[1]
  const tree = m.ind.populations.find((p) => p.key === current?.key)?.tree
  const dropped = current ? current.before.filter((p) => !current.list.includes(p)) : []
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <Settings m={m} inline />
        <Mapping />
      </section>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <div className={css({ display: 'flex', justifyContent: 'space-between', flexWrap: 'wrap', gap: '2', alignItems: 'center' })}>
          <PeriodPicker m={m} />
          <select className={input} value={band} aria-label="Disaggregation band" onChange={(e) => setBand(e.target.value)}>
            <option value="">All patients</option>
            {bands.map((b) => (
              <option key={b.key}>{b.key}</option>
            ))}
          </select>
          {m.ind.kind === 'proportion' ? (
            <strong>
              {pct(m.r)}% ({m.r.numerator.length}/{m.r.denominator.length})
            </strong>
          ) : null}
        </div>
        {steps.map((s) => (
          <button key={s.key} type="button" disabled={s.key === 'period'} onClick={() => s.key !== 'period' && setStep(s.key)} className={css({ display: 'grid', gridTemplateColumns: '9rem 1fr 2rem', gap: '2', alignItems: 'center', cursor: 'pointer', textAlign: 'left', _disabled: { cursor: 'default' } })}>
            <span className={cx(css({ fontSize: 'sm' }), step === s.key ? css({ fontWeight: 'semibold' }) : undefined)}>{s.label}</span>
            <span className={css({ display: 'block', height: '6', borderRadius: 'sm', bg: s.key === 'exclusion' ? 'danger.text' : step === s.key ? 'fg.default' : 'fg.muted', opacity: s.key === 'exclusion' ? 0.5 : 1 })} style={{ width: `${(100 * s.list.length) / max}%`, minWidth: '2px' }} />
            <span>{s.list.length}</span>
          </button>
        ))}
      </section>
      {current && tree ? (
        <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 1fr' }, gap: '3', alignItems: 'start' })}>
          <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h2 className={h2}>{current.label}: criteria</h2>
            <TreeEditor node={tree} onChange={(n) => m.setTree(current.key as PopKey, n)} />
            <code className={mono}>{fhirpath(tree)}</code>
            <Tests m={m} />
          </section>
          <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h3 className={css({ fontWeight: 'semibold' })}>Let through ({current.list.length})</h3>
            <Patients m={m} list={current.list} pop={current.key as PopKey} />
            <h3 className={css({ fontWeight: 'semibold' })}>Dropped here ({dropped.length})</h3>
            <ul className={css({ fontSize: 'sm' })}>
              {dropped.length === 0 ? <li className={muted}>Nobody.</li> : null}
              {dropped.map((p) => (
                <li key={p.id}>
                  {p.name}: {failing(tree, p).join('; ') || 'excluded'}
                </li>
              ))}
            </ul>
          </section>
        </div>
      ) : null}
    </div>
  )
}

/** The conditions that failed for a patient, for "dropped here". */
function failing(n: Node, p: Patient): string[] {
  if (n.kind === 'cond') return holds(n, p) ? [] : [`${condText(n)} is false`]
  return n.children.flatMap((ch) => failing(ch, p))
}

// --- The shell -----------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'Definition left, Try beside it', View: VariantA },
  { key: 'B', name: 'Results first, click a cell to edit', View: VariantB },
  { key: 'C', name: 'A funnel of populations', View: VariantC },
] as const

export function IndicatorEditorPrototype() {
  const m = useIndicator()
  const [variant, setVariant] = useState(() => new URLSearchParams(window.location.search).get('variant') ?? 'A')
  const index = Math.max(
    0,
    VARIANTS.findIndex((v) => v.key === variant),
  )
  const current = VARIANTS[index] ?? VARIANTS[0]
  const go = (step: number) => {
    const next = VARIANTS[(index + step + VARIANTS.length) % VARIANTS.length] ?? VARIANTS[0]
    const url = new URL(window.location.href)
    url.searchParams.set('variant', next.key)
    window.history.replaceState(null, '', url)
    setVariant(next.key)
  }
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest('input, textarea, select, [contenteditable]')) return
      if (e.key === 'ArrowLeft') go(-1)
      if (e.key === 'ArrowRight') go(1)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
  const View = current.View
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', pb: '16' })}>
      <header>
        <p className={muted}>← ug.moh.anc 2.4.0, draft · Indicators</p>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{m.ind.title}</h1>
        <p className={mono}>{m.ind.url}</p>
      </header>
      <View key={current.key} m={m} />
      {import.meta.env.PROD ? null : (
        <div role="toolbar" aria-label="Prototype variants" className={css({ position: 'fixed', bottom: '4', left: '50%', transform: 'translateX(-50%)', display: 'flex', gap: '3', alignItems: 'center', bg: 'fg.default', color: 'bg.canvas', px: '4', py: '2', borderRadius: 'full', boxShadow: 'lg', zIndex: '10' })}>
          <button type="button" aria-label="Previous variant" onClick={() => go(-1)} className={css({ cursor: 'pointer' })}>
            ←
          </button>
          <span>
            {current.key} ({current.name})
          </span>
          <button type="button" aria-label="Next variant" onClick={() => go(1)} className={css({ cursor: 'pointer' })}>
            →
          </button>
        </div>
      )}
    </section>
  )
}
