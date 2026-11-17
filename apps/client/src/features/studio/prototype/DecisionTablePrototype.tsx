// PROTOTYPE (#513), throwaway: three structurally different decision-table editors on
// /prototype/decision-tables, switched with ?variant=A|B|C and the floating bar (← / → keys).
// A: a grid (one column per input, one row per rule, first match wins) with test cases below.
// B: rules as lists of FHIRPath conditions, with a "try a patient" trace that saves test cases.
// C: an outcome matrix over two inputs, showing which rule fires and where nothing does.
// All three edit one in-memory table shaped like the engine's DecisionTable.
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import { CASES, TABLE, conditionText, coverage, describe, evaluate, parseCondition, rowText, type Case, type Condition, type Op, type Row, type Table } from './decisionData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', width: '100%', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const small = cx(ghost, css({ minHeight: 'auto', py: '0', px: '2', fontSize: 'xs' }))
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs' })
const bad = css({ color: 'danger.text' })
const ok = css({ color: 'success.text' })
const cell = css({ px: '2', py: '1', borderBottom: '1px solid', borderColor: 'border', verticalAlign: 'top', fontSize: 'sm' })

function useTable() {
  const [table, setTable] = useState<Table>(TABLE)
  const [cases, setCases] = useState<Case[]>(CASES)
  const setRows = (rows: Row[]) => setTable((t) => ({ ...t, rows }))
  const move = (n: number, by: number) => {
    const rows = [...table.rows]
    const a = rows[n]
    const b = rows[n + by]
    if (!a || !b) return
    rows[n] = b
    rows[n + by] = a
    setRows(rows)
  }
  return { table, setTable, cases, setCases, setRows, move }
}
type Model = ReturnType<typeof useTable>

function Health({ model }: { model: Model }) {
  const { gaps, neverFires } = coverage(model.table)
  const failing = model.cases.filter((c) => evaluate(model.table, c.inputs).outcome !== c.expect).length
  return (
    <div className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' }))}>
      <span>
        <strong>Checks</strong> (the same ones publication runs):
      </span>
      <span className={gaps.length ? bad : ok}>
        {gaps.length ? `${gaps.length} cases match no rule and there is no default, e.g. ${describe(model.table, gaps[0] ?? {})}` : 'Every case matches a rule or the default'}
      </span>
      <span className={neverFires.length ? bad : ok}>{neverFires.length ? `Rule ${neverFires.map((n) => n + 1).join(', ')} never fires: a rule above always matches first` : 'Every rule can fire'}</span>
      <span className={failing ? bad : ok}>{failing ? `${failing} of ${model.cases.length} test cases fail` : `All ${model.cases.length} test cases pass`}</span>
      <label className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
        When no rule matches:
        <select className={cx(input, css({ width: 'auto' }))} value={model.table.default ?? ''} onChange={(e) => model.setTable({ ...model.table, default: e.target.value || null })}>
          <option value="">no outcome (a gap; publication refuses it)</option>
          {model.table.outcomes.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
      </label>
    </div>
  )
}

/** Test cases: inputs, expected and actual outcome. Saved in the package's tests/ folder. */
function Cases({ model, onPick }: { model: Model; onPick?: (c: Case) => void }) {
  const t = model.table
  return (
    <section className={cx(box, css({ p: '3', overflowX: 'auto' }))}>
      <h2 className={h2}>Test cases</h2>
      <table className={css({ width: '100%', borderCollapse: 'collapse' })}>
        <thead>
          <tr>
            {['Case', ...t.inputs.map((i) => i.label), 'Expected', 'Actual'].map((h) => (
              <th key={h} className={cx(cell, css({ textAlign: 'left' }))}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {model.cases.map((c, n) => {
            const r = evaluate(t, c.inputs)
            const pass = r.outcome === c.expect
            return (
              <tr key={n} className={onPick ? css({ cursor: 'pointer' }) : undefined} onClick={() => onPick?.(c)}>
                <td className={cell}>{c.name}</td>
                {t.inputs.map((i) => (
                  <td key={i.id} className={cell}>
                    {c.inputs[i.id]}
                  </td>
                ))}
                <td className={cell}>
                  <select aria-label="Expected" className={cx(input, css({ minHeight: '1.8rem' }))} value={c.expect} onClick={(e) => e.stopPropagation()} onChange={(e) => model.setCases(model.cases.map((x, m) => (m === n ? { ...x, expect: e.target.value } : x)))}>
                    {t.outcomes.map((o) => (
                      <option key={o}>{o}</option>
                    ))}
                  </select>
                </td>
                <td className={cx(cell, pass ? ok : bad)}>
                  {r.outcome ?? 'no outcome'} {r.row !== null ? `(rule ${r.row + 1})` : ''} {pass ? '✓' : '✗'}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </section>
  )
}

// --- Variant A: the grid ------------------------------------------------------------------------

function CellEditor({ model, n, inputId }: { model: Model; n: number; inputId: string }) {
  const t = model.table
  const row = t.rows[n]
  const spec = t.inputs.find((i) => i.id === inputId)
  if (!row || !spec) return null
  const c = row.when.find((x) => x.input === inputId)
  const set = (next: Condition | null) => {
    const when = row.when.filter((x) => x.input !== inputId)
    model.setRows(t.rows.map((r, m) => (m === n ? { ...r, when: next ? [...when, next] : when } : r)))
  }
  const ops: Op[] = spec.type === 'number' ? ['<', '<=', '=', '>=', '>'] : ['=', '!=']
  return (
    <div className={css({ display: 'flex', gap: '1' })}>
      <select aria-label={`${spec.label} test`} className={cx(input, css({ minHeight: '1.8rem', width: '4.5rem' }))} value={c ? c.op : 'any'} onChange={(e) => set(e.target.value === 'any' ? null : { input: inputId, op: e.target.value as Op, value: c?.value ?? (spec.values?.[0] ?? (spec.type === 'boolean' ? 'true' : '0')) })}>
        <option value="any">any</option>
        {ops.map((o) => (
          <option key={o}>{o}</option>
        ))}
      </select>
      {c ? (
        spec.type === 'number' ? (
          <input aria-label={spec.label} type="number" className={cx(input, css({ minHeight: '1.8rem', width: '4rem' }))} value={c.value} onChange={(e) => set({ ...c, value: e.target.value })} />
        ) : (
          <select aria-label={spec.label} className={cx(input, css({ minHeight: '1.8rem' }))} value={c.value} onChange={(e) => set({ ...c, value: e.target.value })}>
            {(spec.type === 'boolean' ? ['true', 'false'] : (spec.values ?? [])).map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        )
      ) : null}
    </div>
  )
}

function VariantA({ model }: { model: Model }) {
  const t = model.table
  const { neverFires } = coverage(t)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <Health model={model} />
      <section className={cx(box, css({ p: '3', overflowX: 'auto' }))}>
        <h2 className={h2}>
          {t.title} <span className={muted}>first matching rule wins, from the top</span>
        </h2>
        <table className={css({ width: '100%', borderCollapse: 'collapse' })}>
          <thead>
            <tr>
              <th className={cell}>#</th>
              {t.inputs.map((i) => (
                <th key={i.id} className={cx(cell, css({ textAlign: 'left' }))}>
                  {i.label}
                  <div className={mono}>{i.path}</div>
                </th>
              ))}
              <th className={cx(cell, css({ textAlign: 'left' }))}>Outcome</th>
              <th className={cell} />
            </tr>
          </thead>
          <tbody>
            {t.rows.map((r, n) => (
              <tr key={n} className={neverFires.includes(n) ? css({ bg: 'bg.subtle' }) : undefined}>
                <td className={cell}>
                  {n + 1}
                  {neverFires.includes(n) ? <div className={bad}>never fires</div> : null}
                </td>
                {t.inputs.map((i) => (
                  <td key={i.id} className={cell}>
                    <CellEditor model={model} n={n} inputId={i.id} />
                  </td>
                ))}
                <td className={cell}>
                  <select aria-label="Outcome" className={cx(input, css({ minHeight: '1.8rem' }))} value={r.outcome} onChange={(e) => model.setRows(t.rows.map((x, m) => (m === n ? { ...x, outcome: e.target.value } : x)))}>
                    {t.outcomes.map((o) => (
                      <option key={o}>{o}</option>
                    ))}
                  </select>
                </td>
                <td className={cx(cell, css({ whiteSpace: 'nowrap' }))}>
                  <button type="button" className={small} aria-label="Move up" disabled={n === 0} onClick={() => model.move(n, -1)}>
                    ▲
                  </button>
                  <button type="button" className={small} aria-label="Move down" disabled={n === t.rows.length - 1} onClick={() => model.move(n, 1)}>
                    ▼
                  </button>
                  <button type="button" className={small} aria-label="Delete rule" onClick={() => model.setRows(t.rows.filter((_, m) => m !== n))}>
                    ✕
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <button type="button" className={cx(ghost, css({ mt: '2' }))} onClick={() => model.setRows([...t.rows, { when: [], outcome: t.outcomes[0] ?? '' }])}>
          Add rule
        </button>
      </section>
      <Cases model={model} />
    </div>
  )
}

// --- Variant B: FHIRPath rules with a trace -------------------------------------------------------

function VariantB({ model }: { model: Model }) {
  const t = model.table
  const [sample, setSample] = useState<Record<string, string>>({ category: 'urgent', age: '30', danger: 'false', pregnant: 'false' })
  const [name, setName] = useState('')
  const result = evaluate(t, sample)
  const editCondition = (n: number, k: number, text: string) => {
    const parsed = parseCondition(t, text)
    if (!parsed) return
    model.setRows(t.rows.map((r, m) => (m === n ? { ...r, when: r.when.map((c, j) => (j === k ? parsed : c)) } : r)))
  }
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 24rem' }, gap: '3', alignItems: 'start' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <Health model={model} />
        <p className={muted}>Conditions are FHIRPath over: {t.inputs.map((i) => i.path).join(', ')}. All conditions in a rule must hold; the first rule that holds wins.</p>
        <ol className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          {t.rows.map((r, n) => {
            const trace = result.trace.find((x) => x.row === n)
            const fired = result.row === n
            return (
              <li key={n} className={cx(box, css({ p: '2', display: 'flex', flexDirection: 'column', gap: '1' }), fired ? css({ borderColor: 'accent.default', bg: 'bg.selected' }) : undefined)}>
                <div className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
                  <strong>Rule {n + 1}</strong>
                  <span className={muted}>{fired ? 'fires for this patient' : trace ? `does not hold: ${trace.failed.join('; ')}` : 'not reached'}</span>
                  <span className={css({ ml: 'auto', display: 'flex', gap: '1' })}>
                    <button type="button" className={small} disabled={n === 0} onClick={() => model.move(n, -1)}>
                      ▲
                    </button>
                    <button type="button" className={small} disabled={n === t.rows.length - 1} onClick={() => model.move(n, 1)}>
                      ▼
                    </button>
                  </span>
                </div>
                {r.when.map((c, k) => (
                  <input key={`${n}-${k}-${conditionText(t, c)}`} aria-label={`Rule ${n + 1} condition ${k + 1}`} className={cx(input, mono)} defaultValue={conditionText(t, c)} onBlur={(e) => editCondition(n, k, e.target.value)} />
                ))}
                <div className={css({ display: 'flex', gap: '2', alignItems: 'center', fontSize: 'sm' })}>
                  then
                  <select className={cx(input, css({ width: 'auto' }))} value={r.outcome} onChange={(e) => model.setRows(t.rows.map((x, m) => (m === n ? { ...x, outcome: e.target.value } : x)))}>
                    {t.outcomes.map((o) => (
                      <option key={o}>{o}</option>
                    ))}
                  </select>
                </div>
              </li>
            )
          })}
        </ol>
      </div>
      <aside className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2', position: { lg: 'sticky' }, top: '4' }))}>
        <h2 className={h2}>Try a patient</h2>
        {t.inputs.map((i) => (
          <label key={i.id} className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '1' })}>
            {i.label}
            {i.type === 'number' ? (
              <input type="number" className={input} value={sample[i.id] ?? ''} onChange={(e) => setSample({ ...sample, [i.id]: e.target.value })} />
            ) : (
              <select className={input} value={sample[i.id] ?? ''} onChange={(e) => setSample({ ...sample, [i.id]: e.target.value })}>
                {(i.type === 'boolean' ? ['true', 'false'] : (i.values ?? [])).map((v) => (
                  <option key={v}>{v}</option>
                ))}
              </select>
            )}
          </label>
        ))}
        <p className={result.outcome ? ok : bad}>{result.outcome ? `Outcome: ${result.outcome} (rule ${(result.row ?? 0) + 1})` : 'No rule matches and there is no default'}</p>
        <div className={css({ display: 'flex', gap: '2' })}>
          <input className={input} placeholder="Name this case" value={name} onChange={(e) => setName(e.target.value)} />
          <button type="button" className={ghost} disabled={!name || !result.outcome} onClick={() => (model.setCases([...model.cases, { name, inputs: sample, expect: result.outcome ?? '' }]), setName(''))}>
            Save as test
          </button>
        </div>
        <span className={muted}>Saves today's outcome as the expected one; change it in the list if today's is wrong.</span>
        <ul className={css({ fontSize: 'sm' })}>
          {model.cases.map((c) => {
            const r = evaluate(t, c.inputs)
            return (
              <li key={c.name}>
                <button type="button" className={css({ cursor: 'pointer', textAlign: 'left' })} onClick={() => setSample(c.inputs)}>
                  {r.outcome === c.expect ? <span className={ok}>✓</span> : <span className={bad}>✗</span>} {c.name}: expects {c.expect}
                </button>
              </li>
            )
          })}
        </ul>
      </aside>
    </div>
  )
}

// --- Variant C: the outcome matrix -----------------------------------------------------------------

function VariantC({ model }: { model: Model }) {
  const t = model.table
  const [rowAxis, setRowAxis] = useState('category')
  const [colAxis, setColAxis] = useState('age')
  const [fixed, setFixed] = useState<Record<string, string>>({ category: 'urgent', age: '30', danger: 'false', pregnant: 'false' })
  const domain = (id: string) => {
    const i = t.inputs.find((x) => x.id === id)
    return i?.type === 'code' ? (i.values ?? []) : i?.type === 'boolean' ? ['true', 'false'] : (i?.samples ?? []).map(String)
  }
  const label = (id: string) => t.inputs.find((x) => x.id === id)?.label ?? id
  const addRule = (sample: Record<string, string>) => {
    // The new rule goes first, for exactly this cell; its outcome is then chosen in the list.
    model.setRows([{ when: [rowAxis, colAxis].map((id) => ({ input: id, op: '=' as Op, value: sample[id] ?? '' })), outcome: t.outcomes[0] ?? '' }, ...t.rows])
  }
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 22rem' }, gap: '3', alignItems: 'start' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <Health model={model} />
        <section className={cx(box, css({ p: '3', overflowX: 'auto' }))}>
          <div className={css({ display: 'flex', gap: '3', flexWrap: 'wrap', fontSize: 'sm', mb: '2' })}>
            <label>
              Down:{' '}
              <select className={cx(input, css({ width: 'auto' }))} value={rowAxis} onChange={(e) => setRowAxis(e.target.value)}>
                {t.inputs.map((i) => (
                  <option key={i.id} value={i.id} disabled={i.id === colAxis}>
                    {i.label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Across:{' '}
              <select className={cx(input, css({ width: 'auto' }))} value={colAxis} onChange={(e) => setColAxis(e.target.value)}>
                {t.inputs.map((i) => (
                  <option key={i.id} value={i.id} disabled={i.id === rowAxis}>
                    {i.label}
                  </option>
                ))}
              </select>
            </label>
            {t.inputs
              .filter((i) => i.id !== rowAxis && i.id !== colAxis)
              .map((i) => (
                <label key={i.id}>
                  {i.label}:{' '}
                  <select className={cx(input, css({ width: 'auto' }))} value={fixed[i.id] ?? ''} onChange={(e) => setFixed({ ...fixed, [i.id]: e.target.value })}>
                    {domain(i.id).map((v) => (
                      <option key={v}>{v}</option>
                    ))}
                  </select>
                </label>
              ))}
          </div>
          <table className={css({ borderCollapse: 'collapse' })}>
            <thead>
              <tr>
                <th className={cell}>
                  {label(rowAxis)} \ {label(colAxis)}
                </th>
                {domain(colAxis).map((v) => (
                  <th key={v} className={cell}>
                    {v}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {domain(rowAxis).map((rv) => (
                <tr key={rv}>
                  <th className={cx(cell, css({ textAlign: 'left' }))}>{rv}</th>
                  {domain(colAxis).map((cv) => {
                    const sample = { ...fixed, [rowAxis]: rv, [colAxis]: cv }
                    const r = evaluate(t, sample)
                    return (
                      <td key={cv} className={cx(cell, css({ textAlign: 'center', minWidth: '6rem' }), r.outcome === null ? css({ bg: 'bg.subtle', color: 'danger.text' }) : r.outcome === 'bypass' ? css({ bg: 'bg.selected' }) : undefined)}>
                        {r.outcome ?? 'nothing'}
                        <div className={muted}>{r.row !== null ? `rule ${r.row + 1}` : 'no rule'}</div>
                        {r.outcome === null ? (
                          <button type="button" className={small} onClick={() => addRule(sample)}>
                            Add a rule for this
                          </button>
                        ) : null}
                      </td>
                    )
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <Cases model={model} onPick={(c) => setFixed(c.inputs)} />
      </div>
      <aside className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' }))}>
        <h2 className={h2}>Rules, in order</h2>
        {t.rows.map((r, n) => (
          <div key={n} className={css({ display: 'flex', gap: '2', alignItems: 'baseline' })}>
            <strong>{n + 1}.</strong>
            <span className={cx(mono, css({ flex: '1' }))}>{rowText(t, r) || 'always'}</span>
            <select aria-label={`Rule ${n + 1} outcome`} className={cx(input, css({ width: 'auto', minHeight: '1.6rem' }))} value={r.outcome} onChange={(e) => model.setRows(t.rows.map((x, m) => (m === n ? { ...x, outcome: e.target.value } : x)))}>
              {t.outcomes.map((o) => (
                <option key={o}>{o}</option>
              ))}
            </select>
            <button type="button" className={small} disabled={n === 0} onClick={() => model.move(n, -1)}>
              ▲
            </button>
          </div>
        ))}
      </aside>
    </div>
  )
}

// --- The shell -----------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'A grid, one column per input', View: VariantA },
  { key: 'B', name: 'FHIRPath rules with a patient trace', View: VariantB },
  { key: 'C', name: 'An outcome matrix over two inputs', View: VariantC },
] as const

export function DecisionTablePrototype() {
  const model = useTable()
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
        <p className={muted}>← OPD (ug.moh.opd 2.1.0, draft) · OPD visit workflow · Decision table</p>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{model.table.title}</h1>
        <span className={muted}>
          Used by the guard on “Triage → Consultation” (outcome bypass). Inputs come from the workflow's declared data requirements.
        </span>
      </header>
      <View key={current.key} model={model} />
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
