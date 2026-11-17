// PROTOTYPE (#534, #539), throwaway: the indicator editor on /prototype/indicator-editor.
// Variant A as decided in #534 (definition left, Try right; a count until "+ Add denominator";
// the numerator within the denominator; "is in another indicator" conditions), refined in #539:
// - "+ date" conditions on many kinds of date, relative to the reporting period (in the period,
//   on, before, after, between, within N days/weeks/months, in the previous N periods, empty,
//   not empty), and the period date chosen from the same list;
// - age bands the author defines (from–to, years or months, age at period end or at the period date);
// - disaggregation by any coded value, its bands taken from the value's codes.
// Earlier rounds (variants B and C, "starts from", "+ in the denominator") are in this branch's history.
import { css, cx } from '@health/design-system/css'
import { useState } from 'react'

import {
  CODED,
  DATE_FIELDS,
  DATE_OPS,
  HMIS_MAPPING,
  INITIAL,
  INPUTS,
  NATIONAL,
  DEFAULT_PERIOD,
  PERIOD_TYPES,
  bandLabel,
  bands,
  baseLabel,
  baseOf,
  compiled,
  disaggLabel,
  display,
  evaluate,
  holds,
  newDate,
  periodEnd,
  periodStart,
  refOptions,
  startOf,
  type Anchor,
  type Base,
  type Ctx,
  type DateCond,
  type DateField,
  type DateOp,
  type Disagg,
  type Indicator,
  type Join,
  type NationalId,
  type Node,
  type Op,
  type Patient,
  type Period,
  type PeriodType,
  type SavedTest,
  type Side,
  type Unit,
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
const row = css({ display: 'flex', gap: '1', alignItems: 'center', flexWrap: 'wrap', fontSize: 'sm' })
const OPS: Op[] = ['=', '!=', '<', '<=', '>', '>=']
const JOINS: Record<Join, string> = { all: 'all of these', any: 'any of these', not: 'none of these' }

const enc = (b: Base) => (b.kind === 'indicator' ? `indicator:${b.id}:${b.side}` : b.kind)
const dec = (s: string): Base => {
  const [k, id, side] = s.split(':')
  return k === 'indicator' && id && side ? { kind: 'indicator', id, side: side as Side } : k === 'denominator' ? { kind: 'denominator' } : { kind: 'period' }
}

// --- Conditions --------------------------------------------------------------------------------------

function AnchorPicker({ a, onChange }: { a: Anchor; onChange: (a: Anchor) => void }) {
  return (
    <>
      <select className={input} value={a.at} aria-label="Date" onChange={(e) => onChange({ at: e.target.value as Anchor['at'], date: a.date ?? '2026-01-01' })}>
        <option value="periodStart">the start of the reporting period</option>
        <option value="periodEnd">the end of the reporting period</option>
        <option value="fixed">a fixed date</option>
      </select>
      {a.at === 'fixed' ? <input type="date" className={input} value={a.date ?? ''} aria-label="Fixed date" onChange={(e) => onChange({ at: 'fixed', date: e.target.value })} /> : null}
    </>
  )
}

function DateEditor({ node, onChange }: { node: DateCond; onChange: (n: DateCond) => void }) {
  const n = node
  return (
    <div className={row}>
      <select className={input} value={n.field} aria-label="Which date" onChange={(e) => onChange({ ...n, field: e.target.value as DateField })}>
        {DATE_FIELDS.map((f) => (
          <option key={f.id} value={f.id}>
            {f.label}
          </option>
        ))}
      </select>
      <select className={input} value={n.op} aria-label="Date operator" onChange={(e) => onChange({ ...n, op: e.target.value as DateOp })}>
        {DATE_OPS.map((o) => (
          <option key={o.id} value={o.id}>
            {o.label}
          </option>
        ))}
      </select>
      {n.op === 'on' || n.op === 'before' || n.op === 'after' ? <AnchorPicker a={n.a} onChange={(a) => onChange({ ...n, a })} /> : null}
      {n.op === 'between' ? (
        <>
          <AnchorPicker a={n.a} onChange={(a) => onChange({ ...n, a })} />
          and
          <AnchorPicker a={n.b} onChange={(b) => onChange({ ...n, b })} />
        </>
      ) : null}
      {n.op === 'within' ? (
        <>
          <input className={cx(input, css({ width: '4rem' }))} type="number" min={0} value={n.n} aria-label="How many" onChange={(e) => onChange({ ...n, n: Number(e.target.value) })} />
          <select className={input} value={n.unit} aria-label="Unit" onChange={(e) => onChange({ ...n, unit: e.target.value as Unit })}>
            <option>days</option>
            <option>weeks</option>
            <option>months</option>
          </select>
          <select className={input} value={n.dir} aria-label="Before or after" onChange={(e) => onChange({ ...n, dir: e.target.value as 'before' | 'after' })}>
            <option value="before">before</option>
            <option value="after">after</option>
          </select>
          <AnchorPicker a={n.a} onChange={(a) => onChange({ ...n, a })} />
        </>
      ) : null}
      {n.op === 'previous' ? (
        <>
          <input className={cx(input, css({ width: '4rem' }))} type="number" min={1} value={n.n} aria-label="How many periods" onChange={(e) => onChange({ ...n, n: Number(e.target.value) })} />
          reporting periods
        </>
      ) : null}
    </div>
  )
}

function TreeEditor({ node, onChange, patient, ctx, depth = 0 }: { node: Node; onChange: (n: Node) => void; patient?: Patient | undefined; ctx?: Ctx | undefined; depth?: number }) {
  const mark = patient && ctx ? (holds(node, patient, ctx) ? <span className={ok}>✓ </span> : <span className={bad}>✗ </span>) : null
  if (node.kind === 'date') {
    return (
      <div className={row}>
        {mark}
        <DateEditor node={node} onChange={onChange} />
      </div>
    )
  }
  if (node.kind === 'member') {
    return (
      <div className={row}>
        {mark}
        is in
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
      <div className={row}>
        {mark}
        <select className={input} value={node.input} aria-label="Input" onChange={(e) => onChange({ ...node, input: e.target.value, value: INPUTS.find((i) => i.id === e.target.value)?.values?.[0] ?? '' })}>
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
  const add = (n: Node) => onChange({ ...node, children: [...node.children, n] })
  return (
    <div className={css({ borderLeft: depth ? '2px solid' : 'none', borderColor: 'border', pl: depth ? '3' : '0', display: 'flex', flexDirection: 'column', gap: '1' })}>
      <div className={row}>
        {mark}
        <select className={input} value={node.join} aria-label="Join" onChange={(e) => onChange({ ...node, join: e.target.value as Join })}>
          {(Object.keys(JOINS) as Join[]).map((j) => (
            <option key={j} value={j}>
              {JOINS[j]}
            </option>
          ))}
        </select>
        <button type="button" className={small} onClick={() => add({ kind: 'cond', input: 'age', op: '>=', value: '15' })}>
          + condition
        </button>
        <button type="button" className={small} onClick={() => add(newDate())}>
          + date
        </button>
        <button type="button" className={small} onClick={() => add({ kind: 'group', join: 'any', children: [] })}>
          + group
        </button>
        <button type="button" className={small} onClick={() => add({ kind: 'member', ref: refOptions()[0] ?? { kind: 'period' } })}>
          + in another indicator
        </button>
      </div>
      {node.children.map((ch, i) => (
        <div key={i} className={css({ display: 'flex', gap: '1', alignItems: 'flex-start' })}>
          <div className={css({ flex: '1' })}>
            <TreeEditor node={ch} onChange={(n) => set(i, n)} patient={patient} ctx={ctx} depth={depth + 1} />
          </div>
          <button type="button" className={small} aria-label="Remove" onClick={() => set(i, null)}>
            ×
          </button>
        </div>
      ))}
    </div>
  )
}

// --- Sides ---------------------------------------------------------------------------------------------

function SideCard({ ind, side, setInd, count }: { ind: Indicator; side: Side; setInd: (i: Indicator) => void; count: number | null }) {
  const part = side === 'numerator' ? ind.numerator : ind.denominator
  const set = (p: { tree: Node } | 'one') => (side === 'numerator' ? p !== 'one' && setInd({ ...ind, numerator: p }) : setInd({ ...ind, denominator: p }))
  if (part === 'one') {
    return (
      <div className={css({ display: 'flex', gap: '3', alignItems: 'center' })}>
        <button type="button" className={ghost} onClick={() => set({ tree: { kind: 'group', join: 'all', children: [] } })}>
          + Add denominator
        </button>
        <span className={muted}>Without one, the indicator is a count of the numerator.</span>
      </div>
    )
  }
  const within = side === 'numerator' && ind.denominator !== 'one'
  return (
    <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
      <div className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', gap: '2' })}>
        <h2 className={h2}>{side === 'numerator' ? 'Numerator' : 'Denominator'}</h2>
        <span className={css({ display: 'flex', gap: '2', alignItems: 'baseline' })}>
          {count === null ? null : <span className={muted}>{count} in the period tried</span>}
          {side === 'denominator' ? (
            <button type="button" className={small} onClick={() => set('one')}>
              Remove denominator
            </button>
          ) : null}
        </span>
      </div>
      <span className={css({ fontSize: 'sm' })}>{within ? 'Patients in the denominator who match' : 'Patients with care in the period who match'}</span>
      <TreeEditor node={part.tree} onChange={(tree) => set({ tree })} />
      <code className={mono}>{compiled(ind, side)}</code>
    </section>
  )
}

// --- Period date and disaggregations ------------------------------------------------------------------------

function AgeBandsEditor({ d, onChange }: { d: Extract<Disagg, { kind: 'age' }>; onChange: (d: Disagg) => void }) {
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', pl: '3' })}>
      <div className={row}>
        <input className={input} value={d.label} aria-label="Name" onChange={(e) => onChange({ ...d, label: e.target.value })} />
        in
        <select className={input} value={d.unit} aria-label="Unit" onChange={(e) => onChange({ ...d, unit: e.target.value as 'years' | 'months' })}>
          <option>years</option>
          <option>months</option>
        </select>
        age at
        <select className={input} value={d.at} aria-label="Age at" onChange={(e) => onChange({ ...d, at: e.target.value as 'periodEnd' | 'periodDate' })}>
          <option value="periodEnd">the end of the reporting period</option>
          <option value="periodDate">the period date</option>
        </select>
      </div>
      {d.bands.map((b, i) => (
        <div key={i} className={row}>
          <input className={cx(input, css({ width: '4rem' }))} type="number" min={0} value={b.from} aria-label="From" onChange={(e) => onChange({ ...d, bands: d.bands.map((x, j) => (j === i ? { ...x, from: Number(e.target.value) } : x)) })} />
          to
          <input className={cx(input, css({ width: '4rem' }))} type="number" min={0} value={b.to ?? ''} placeholder="and over" aria-label="To" onChange={(e) => onChange({ ...d, bands: d.bands.map((x, j) => (j === i ? { ...x, to: e.target.value === '' ? null : Number(e.target.value) } : x)) })} />
          <span className={muted}>{bandLabel(b, d.unit)}</span>
          <button type="button" className={small} aria-label="Remove band" onClick={() => onChange({ ...d, bands: d.bands.filter((_, j) => j !== i) })}>
            ×
          </button>
        </div>
      ))}
      <button type="button" className={cx(small, css({ alignSelf: 'flex-start' }))} onClick={() => onChange({ ...d, bands: [...d.bands, { from: (d.bands.at(-1)?.to ?? 0) + 1, to: null }] })}>
        + band
      </button>
    </div>
  )
}

function Settings({ ind, setInd }: { ind: Indicator; setInd: (i: Indicator) => void }) {
  const setD = (i: number, d: Disagg | null) => setInd({ ...ind, disaggregations: d ? ind.disaggregations.map((x, j) => (j === i ? d : x)) : ind.disaggregations.filter((_, j) => j !== i) })
  const addD = (d: Disagg) => setInd({ ...ind, disaggregations: [...ind.disaggregations, d] })
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3', fontSize: 'sm' })}>
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        Period date: which date puts a patient in a reporting period (required)
        <select className={input} value={ind.periodDate} onChange={(e) => setInd({ ...ind, periodDate: e.target.value as DateField })}>
          {DATE_FIELDS.map((d) => (
            <option key={d.id} value={d.id}>
              {d.label}
            </option>
          ))}
        </select>
      </label>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <span>Disaggregations</span>
        {ind.disaggregations.map((d, i) => (
          <div key={i} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
            <div className={row}>
              <strong>{disaggLabel(d)}</strong>
              {d.kind === 'national' ? <span className={muted}>{NATIONAL[d.id].bands.map((b) => bandLabel(b, 'y')).join(', ')}</span> : null}
              {d.kind === 'coded' ? <span className={muted}>{(INPUTS.find((x) => x.id === d.input)?.values ?? ['true', 'false']).join(', ')}</span> : null}
              <button type="button" className={small} aria-label="Remove disaggregation" onClick={() => setD(i, null)}>
                ×
              </button>
            </div>
            {d.kind === 'age' ? <AgeBandsEditor d={d} onChange={(x) => setD(i, x)} /> : null}
          </div>
        ))}
        <div className={row}>
          <select className={input} value="" aria-label="Add national age bands" onChange={(e) => e.target.value && addD({ kind: 'national', id: e.target.value as NationalId })}>
            <option value="">+ national age bands…</option>
            {(Object.keys(NATIONAL) as NationalId[]).map((id) => (
              <option key={id} value={id}>
                {NATIONAL[id].label}
              </option>
            ))}
          </select>
          <button type="button" className={small} onClick={() => addD({ kind: 'age', label: 'Under-5 bands', unit: 'months', at: 'periodEnd', bands: [{ from: 0, to: 11 }, { from: 12, to: 59 }] })}>
            + age bands
          </button>
          <select className={input} value="" aria-label="Add a coded disaggregation" onChange={(e) => e.target.value && addD({ kind: 'coded', input: e.target.value })}>
            <option value="">+ by a coded value…</option>
            {CODED.map((i) => (
              <option key={i.id} value={i.id}>
                {i.label}
              </option>
            ))}
          </select>
        </div>
      </div>
    </div>
  )
}

// --- Try -----------------------------------------------------------------------------------------------

/** Patients on one side, each opening to that side's conditions marked for them. */
function Patients({ ind, side, list, setInd, period }: { ind: Indicator; side: Side; list: Patient[]; setInd: (i: Indicator) => void; period: Period }) {
  const [open, setOpen] = useState<string | null>(null)
  const ctx: Ctx = { ind, period, seen: [`${ind.id}.${side}`] }
  const part = side === 'numerator' ? ind.numerator : ind.denominator
  return (
    <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
      {list.length === 0 ? <li className={muted}>Nobody.</li> : null}
      {list.map((p) => (
        <li key={p.id}>
          <button type="button" className={css({ cursor: 'pointer', textAlign: 'left' })} onClick={() => setOpen(open === p.id ? null : p.id)}>
            {open === p.id ? '▾' : '▸'} {p.name} · {p.encounterType} · GA {p.gestationWeeks}w · HIV {p.hiv} · visit {p.dates.start}
            {p.dates.recorded !== p.dates.start ? <span className={muted}> (entered {p.dates.recorded})</span> : null}
          </button>
          {open === p.id && part !== 'one' ? (
            <div className={cx(box, css({ p: '2', mt: '1', bg: 'bg.subtle' }))}>
              <TreeEditor node={part.tree} onChange={(tree) => setInd(side === 'numerator' ? { ...ind, numerator: { tree } } : { ...ind, denominator: { tree } })} patient={p} ctx={ctx} />
            </div>
          ) : null}
        </li>
      ))}
    </ul>
  )
}

function Dropped({ ind, side, period }: { ind: Indicator; side: Side; period: Period }) {
  const part = side === 'numerator' ? ind.numerator : ind.denominator
  if (part === 'one') return null
  const from = baseOf(ind, startOf(ind, side), period)
  const out = from.filter((p) => !holds(part.tree, p, { ind, period, seen: [`${ind.id}.${side}`] }))
  return (
    <p className={muted}>
      {from.length} {side === 'numerator' && ind.denominator !== 'one' ? 'in the denominator' : 'with care in the period'}; {out.length} don't match
      {out.length ? `: ${out.map((p) => p.name).join(', ')}` : ''}.
    </p>
  )
}

export function IndicatorEditorPrototype() {
  const [ind, setInd] = useState<Indicator>(INITIAL)
  const [period, setPeriod] = useState<Period>(DEFAULT_PERIOD)
  const [drill, setDrill] = useState<Side>('numerator')
  const [tests, setTests] = useState<SavedTest[]>([])
  const r = evaluate(ind, period)
  const sides: Side[] = r.denominator === null ? ['numerator'] : ['numerator', 'denominator']
  const shown: Side = sides.includes(drill) ? drill : 'numerator'
  const expectNow = { numerator: r.numerator.length, denominator: r.denominator === null ? ('one' as const) : r.denominator.length }
  const pass = (t: SavedTest) => {
    const x = evaluate(ind, t.period)
    return t.expect.numerator === x.numerator.length && t.expect.denominator === (x.denominator === null ? 'one' : x.denominator.length)
  }
  const line = (key: string, label: string, test: (p: Patient) => boolean, strong = false) => {
    const n = r.numerator.filter(test).length
    const d = r.denominator?.filter(test).length
    return (
      <tr key={key}>
        <td className={strong ? css({ fontWeight: 'semibold' }) : cx(muted, css({ pl: '3' }))}>{label}</td>
        <td>{n}</td>
        {d === undefined ? null : <td>{d}</td>}
        {d === undefined ? null : <td>{d ? `${Math.round((100 * n) / d)}%` : '–'}</td>}
      </tr>
    )
  }
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', pb: '8' })}>
      <header>
        <p className={muted}>← ug.moh.anc 2.4.0, draft · Indicators</p>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{ind.title}</h1>
        <p className={mono}>https://health.go.ug/fhir/Measure/{ind.id}</p>
      </header>
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 27rem' }, gap: '4', alignItems: 'start' })}>
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
          <span className={muted}>A report chooses the period; try the indicator as a report would.</span>
          <div className={row}>
            <select className={input} value={period.type} aria-label="Period type" onChange={(e) => setPeriod(PERIOD_TYPES.find((t) => t.id === (e.target.value as PeriodType))?.periods.at(-1) ?? DEFAULT_PERIOD)}>
              {PERIOD_TYPES.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.label}
                </option>
              ))}
            </select>
            <select className={input} value={period.label} aria-label="Period" onChange={(e) => setPeriod(PERIOD_TYPES.find((t) => t.id === period.type)?.periods.find((p) => p.label === e.target.value) ?? period)}>
              {(PERIOD_TYPES.find((t) => t.id === period.type)?.periods ?? []).map((p) => (
                <option key={p.label}>{p.label}</option>
              ))}
            </select>
          </div>
          <span className={muted}>
            Reporting period {periodStart(period)} to {periodEnd(period)}
          </span>
          <p className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
            {display(r)} {r.denominator === null ? <span className={muted}>(a count)</span> : <span className={muted}>({r.numerator.length} of {r.denominator.length})</span>}
          </p>
          <table className={css({ fontSize: 'sm', '& td, & th': { px: '1', textAlign: 'right' }, '& th:first-child, & td:first-child': { textAlign: 'left' } })}>
            <thead>
              <tr>
                <th />
                {sides.map((s) => (
                  <th key={s}>
                    <button type="button" className={cx(css({ cursor: 'pointer' }), shown === s ? css({ textDecoration: 'underline' }) : undefined)} onClick={() => setDrill(s)}>
                      {s === 'numerator' ? 'Numerator' : 'Denominator'}
                    </button>
                  </th>
                ))}
                {r.denominator === null ? null : <th>%</th>}
              </tr>
            </thead>
            <tbody>
              {line('all', 'All', () => true, true)}
              {ind.disaggregations.flatMap((d, i) => [
                <tr key={`h${i}`}>
                  <td colSpan={4} className={css({ fontWeight: 'semibold', pt: '2' })}>
                    {disaggLabel(d)}
                  </td>
                </tr>,
                ...bands(d, ind, period).map((b) => line(`${i}${b.label}`, b.label, b.test)),
              ])}
            </tbody>
          </table>
          <h3 className={css({ fontWeight: 'semibold' })}>{shown === 'numerator' ? 'Numerator' : 'Denominator'}: who and why</h3>
          <Dropped ind={ind} side={shown} period={period} />
          <Patients ind={ind} side={shown} list={shown === 'denominator' ? (r.denominator ?? []) : r.numerator} setInd={setInd} period={period} />
          <button type="button" className={ghost} onClick={() => setTests([...tests.filter((t) => t.period.label !== period.label), { period, expect: expectNow }])}>
            Save as test ({period.label})
          </button>
          {tests.length === 0 ? <span className={muted}>No saved tests. Submission runs them and fails if the counts change.</span> : null}
          {tests.map((t) => (
            <div key={t.period.label} className={cx(css({ fontSize: 'sm' }), pass(t) ? ok : bad)}>
              {pass(t) ? '✓' : '✗ would fail submission:'} {t.period.label} expects numerator {t.expect.numerator}, denominator {t.expect.denominator === 'one' ? '1' : t.expect.denominator}
            </div>
          ))}
        </aside>
      </div>
    </section>
  )
}
