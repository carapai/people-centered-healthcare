// PROTOTYPE (#513), throwaway: variant D, each rule's condition as a tree of groups joined by
// all, any or not (none of), with the FHIRPath it compiles to and a "try a patient" trace that
// marks every node as holding or not.
import { css, cx } from '@health/design-system/css'
import { useState } from 'react'

import { CASES, TABLE, TREE_ROWS, coverageTree, describe, evaluateTree, holdsNode, nodeText, type Case, type Join, type Node, type Op, type Table, type TreeRow } from './decisionData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: '1.8rem', bg: 'bg.surface', fontSize: 'sm' })
const small = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', fontSize: 'xs', cursor: 'pointer', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs' })
const bad = css({ color: 'danger.text' })
const ok = css({ color: 'success.text' })

const JOIN_LABEL: Record<Join, string> = { all: 'all of these', any: 'any of these', not: 'not (none of these)' }

/** Replaces the node at `path` (child indexes from the root) with what `f` returns; null removes it. */
function update(root: Node, path: number[], f: (n: Node) => Node | null): Node {
  if (path.length === 0) return f(root) ?? root
  if (root.kind !== 'group') return root
  const [head, ...rest] = path
  const children = root.children.flatMap((child, i) => {
    if (i !== head) return [child]
    if (rest.length === 0) {
      const next = f(child)
      return next ? [next] : []
    }
    return [update(child, rest, f)]
  })
  return { ...root, children }
}

function NodeEditor({ t, node, path, onChange, sample }: { t: Table; node: Node; path: number[]; onChange: (path: number[], f: (n: Node) => Node | null) => void; sample: Record<string, string> | null }) {
  const held = sample ? holdsNode(t, node, sample) : null
  const mark = held === null ? null : held ? <span className={ok} title="holds for this patient">✓</span> : <span className={bad} title="does not hold for this patient">✗</span>
  if (node.kind === 'cond') {
    const spec = t.inputs.find((i) => i.id === node.c.input)
    const ops: Op[] = spec?.type === 'number' ? ['<', '<=', '=', '!=', '>=', '>'] : ['=', '!=']
    const set = (patch: Partial<typeof node.c>) => onChange(path, () => ({ kind: 'cond', c: { ...node.c, ...patch } }))
    return (
      <div className={css({ display: 'flex', gap: '1', alignItems: 'center', flexWrap: 'wrap' })}>
        {mark}
        <select aria-label="Input" className={input} value={node.c.input} onChange={(e) => set({ input: e.target.value, value: t.inputs.find((i) => i.id === e.target.value)?.values?.[0] ?? 'true' })}>
          {t.inputs.map((i) => (
            <option key={i.id} value={i.id}>
              {i.label}
            </option>
          ))}
        </select>
        <select aria-label="Operator" className={input} value={node.c.op} onChange={(e) => set({ op: e.target.value as Op })}>
          {ops.map((o) => (
            <option key={o}>{o}</option>
          ))}
        </select>
        {spec?.type === 'number' ? (
          <input aria-label="Value" type="number" className={cx(input, css({ width: '5rem' }))} value={node.c.value} onChange={(e) => set({ value: e.target.value })} />
        ) : (
          <select aria-label="Value" className={input} value={node.c.value} onChange={(e) => set({ value: e.target.value })}>
            {(spec?.type === 'boolean' ? ['true', 'false'] : (spec?.values ?? [])).map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        )}
        {path.length > 0 ? (
          <button type="button" className={small} aria-label="Remove condition" onClick={() => onChange(path, () => null)}>
            ✕
          </button>
        ) : null}
      </div>
    )
  }
  const add = (child: Node) => onChange(path, (n) => (n.kind === 'group' ? { ...n, children: [...n.children, child] } : n))
  const first = t.inputs[0]
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', borderLeft: '3px solid', borderColor: node.join === 'not' ? 'danger.text' : node.join === 'any' ? 'accent.default' : 'border', pl: '3', py: '1' })}>
      <div className={css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap' })}>
        {mark}
        <select aria-label="Join" className={cx(input, css({ fontWeight: 'semibold' }))} value={node.join} onChange={(e) => onChange(path, (n) => (n.kind === 'group' ? { ...n, join: e.target.value as Join } : n))}>
          {(['all', 'any', 'not'] as const).map((j) => (
            <option key={j} value={j}>
              {JOIN_LABEL[j]}
            </option>
          ))}
        </select>
        <button type="button" className={small} onClick={() => add({ kind: 'cond', c: { input: first?.id ?? '', op: '=', value: first?.values?.[0] ?? 'true' } })}>
          + condition
        </button>
        <button type="button" className={small} onClick={() => add({ kind: 'group', join: 'all', children: [] })}>
          + group
        </button>
        {path.length > 0 ? (
          <button type="button" className={small} aria-label="Remove group" onClick={() => onChange(path, () => null)}>
            ✕ group
          </button>
        ) : null}
      </div>
      {node.children.length === 0 ? <span className={muted}>Empty: {node.join === 'not' ? 'always holds' : 'always holds'}. Add a condition or a group.</span> : null}
      {node.children.map((child, i) => (
        <NodeEditor key={i} t={t} node={child} path={[...path, i]} onChange={onChange} sample={sample} />
      ))}
    </div>
  )
}

export function VariantD() {
  const [table, setTable] = useState<Table>(TABLE)
  const [rows, setRows] = useState<TreeRow[]>(TREE_ROWS)
  const [cases, setCases] = useState<Case[]>(CASES)
  const [open, setOpen] = useState<number | null>(1)
  const [sample, setSample] = useState<Record<string, string> | null>({ category: 'urgent', age: '3', danger: 'true', pregnant: 'false' })
  const [name, setName] = useState('')
  const { gaps, neverFires } = coverageTree(table, rows)
  const failing = cases.filter((c) => evaluateTree(table, rows, c.inputs).outcome !== c.expect).length
  const result = sample ? evaluateTree(table, rows, sample) : null
  const rule = open === null ? undefined : rows[open]
  const setRule = (patch: Partial<TreeRow>) => open !== null && setRows(rows.map((r, m) => (m === open ? { ...r, ...patch } : r)))
  const edit = (path: number[], f: (node: Node) => Node | null) => rule && setRule({ tree: update(rule.tree, path, f) })
  const move = (n: number, by: number) => {
    const next = [...rows]
    const a = next[n]
    const b = next[n + by]
    if (!a || !b) return
    next[n] = b
    next[n + by] = a
    setRows(next)
    if (open === n) setOpen(n + by)
    else if (open === n + by) setOpen(n)
  }
  const add = () => {
    setRows([...rows, { name: `New rule ${rows.length + 1}`, tree: { kind: 'group', join: 'all', children: [] }, outcome: table.outcomes[0] ?? '' }])
    setOpen(rows.length)
  }
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <div className={cx(box, css({ p: '3', display: 'flex', gap: '4', flexWrap: 'wrap', fontSize: 'sm', alignItems: 'center' }))}>
        <strong>Checks:</strong>
        <span className={gaps.length ? bad : ok}>{gaps.length ? `${gaps.length} cases match no rule, e.g. ${describe(table, gaps[0] ?? {})}` : 'every case is covered'}</span>
        <span className={neverFires.length ? bad : ok}>{neverFires.length ? `“${neverFires.map((n) => rows[n]?.name).join('”, “')}” never fires` : 'every rule can fire'}</span>
        <span className={failing ? bad : ok}>{failing ? `${failing} of ${cases.length} test cases fail` : `all ${cases.length} test cases pass`}</span>
        <label>
          When no rule matches:{' '}
          <select className={input} value={table.default ?? ''} onChange={(e) => setTable({ ...table, default: e.target.value || null })}>
            <option value="">no outcome (a gap)</option>
            {table.outcomes.map((o) => (
              <option key={o}>{o}</option>
            ))}
          </select>
        </label>
      </div>
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', xl: '20rem 1fr 20rem' }, gap: '3', alignItems: 'start' })}>
        <nav aria-label="Rules" className={cx(box, css({ p: '2', display: 'flex', flexDirection: 'column', gap: '1' }))}>
          <span className={muted}>Rules, tried from the top; the first that holds wins</span>
          <ol className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
            {rows.map((r, n) => (
              <li key={n} className={cx(css({ display: 'flex', gap: '1', alignItems: 'stretch', borderRadius: 'sm' }), open === n ? css({ bg: 'bg.selected' }) : undefined)}>
                <button type="button" aria-current={open === n ? 'true' : undefined} onClick={() => setOpen(n)} className={css({ flex: '1', textAlign: 'left', p: '2', cursor: 'pointer', fontSize: 'sm' })}>
                  <div>
                    <strong>{n + 1}.</strong> {r.name || '(unnamed)'}
                  </div>
                  <div className={muted}>
                    → {r.outcome}
                    {result?.row === n ? <span className={ok}> · fires for this patient</span> : null}
                    {neverFires.includes(n) ? <span className={bad}> · never fires</span> : null}
                  </div>
                </button>
                <span className={css({ display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: '0.5', pr: '1' })}>
                  <button type="button" className={small} aria-label={`Move ${r.name} up`} disabled={n === 0} onClick={() => move(n, -1)}>
                    ▲
                  </button>
                  <button type="button" className={small} aria-label={`Move ${r.name} down`} disabled={n === rows.length - 1} onClick={() => move(n, 1)}>
                    ▼
                  </button>
                </span>
              </li>
            ))}
          </ol>
          <button type="button" className={cx(small, css({ py: '1' }))} onClick={add}>
            Add rule
          </button>
        </nav>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '3' }))}>
          {rule && open !== null ? (
            <>
              <div className={css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap' })}>
                <strong>Rule {open + 1}</strong>
                <input aria-label="Rule name" className={cx(input, css({ flex: '1', minWidth: '14rem', fontWeight: 'semibold' }))} value={rule.name} onChange={(e) => setRule({ name: e.target.value })} />
                <span className={css({ fontSize: 'sm' })}>then</span>
                <select aria-label="Outcome" className={input} value={rule.outcome} onChange={(e) => setRule({ outcome: e.target.value })}>
                  {table.outcomes.map((o) => (
                    <option key={o}>{o}</option>
                  ))}
                </select>
                <button type="button" className={small} onClick={() => (setRows(rows.filter((_, m) => m !== open)), setOpen(null))}>
                  Delete rule
                </button>
              </div>
              <NodeEditor t={table} node={rule.tree} path={[]} onChange={edit} sample={sample} />
              <div>
                <div className={muted}>Saved as FHIRPath</div>
                <div className={cx(mono, css({ color: 'fg.muted', bg: 'bg.subtle', p: '2', borderRadius: 'sm', overflowX: 'auto' }))}>{nodeText(table, rule.tree)}</div>
              </div>
            </>
          ) : (
            <p className={muted}>Choose a rule from the list to see and edit it.</p>
          )}
        </section>
        <aside className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2', position: { xl: 'sticky' }, top: '4' }))}>
          <h2 className={h2}>Try a patient</h2>
          <label className={css({ fontSize: 'sm' })}>
            <input type="checkbox" checked={sample !== null} onChange={(e) => setSample(e.target.checked ? { category: 'urgent', age: '30', danger: 'false', pregnant: 'false' } : null)} /> Mark each node ✓ or ✗ for this patient
          </label>
          {sample
            ? table.inputs.map((i) => (
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
              ))
            : null}
          {result ? (
            <p className={result.outcome ? ok : bad}>
              {result.outcome ? (
                <>
                  Outcome: {result.outcome}, from{' '}
                  <button type="button" className={css({ textDecoration: 'underline', cursor: 'pointer' })} onClick={() => setOpen(result.row)}>
                    “{rows[result.row ?? 0]?.name}”
                  </button>
                </>
              ) : (
                'No rule matches and there is no default'
              )}
            </p>
          ) : null}
          {sample ? (
            <div className={css({ display: 'flex', gap: '2' })}>
              <input className={cx(input, css({ flex: '1' }))} placeholder="Name this case" value={name} onChange={(e) => setName(e.target.value)} />
              <button type="button" className={small} disabled={!name || !result?.outcome} onClick={() => (setCases([...cases, { name, inputs: sample, expect: result?.outcome ?? '' }]), setName(''))}>
                Save as test
              </button>
            </div>
          ) : null}
          <h2 className={h2}>Test cases</h2>
          <ul className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '1' })}>
            {cases.map((c, n) => {
              const r = evaluateTree(table, rows, c.inputs)
              const pass = r.outcome === c.expect
              return (
                <li key={n}>
                  <button type="button" className={css({ cursor: 'pointer', textAlign: 'left' })} onClick={() => setSample(c.inputs)}>
                    {pass ? <span className={ok}>✓</span> : <span className={bad}>✗</span>} {c.name}: expects {c.expect}, gets {r.outcome ?? 'nothing'}
                  </button>
                </li>
              )
            })}
          </ul>
        </aside>
      </div>
    </div>
  )
}
