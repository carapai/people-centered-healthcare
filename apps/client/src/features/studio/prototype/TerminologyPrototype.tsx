// PROTOTYPE (#512), throwaway: three structurally different terminology screens on
// /prototype/terminology-studio, switched with ?variant=A|B|C and the floating bar (← / → keys).
// A: a code-system browser where codes are added to the open ValueSet from the tree (a basket).
// B: the ValueSet as include and exclude rules, each with its count, and the expansion below.
// C: a local CodeSystem as a sheet with the semver bump worked out live, and what it reaches.
// Below every variant: the form designer's picker for binding a published ValueSet.
import { css, cx } from '@health/design-system/css'
import { useEffect, useMemo, useState } from 'react'

import { FORMS, SYSTEMS, VALUE_SETS, bump, expand, nextVersion, ruleCodes, systemOf, type CodeSystem, type Concept, type Rule, type ValueSet } from './terminologyData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', width: '100%', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const small = cx(ghost, css({ minHeight: 'auto', py: '0', px: '2', fontSize: 'xs' }))
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs' })
const bad = css({ color: 'danger.text' })
const ok = css({ color: 'success.text' })

export function useTerminology() {
  const [systems, setSystems] = useState<CodeSystem[]>(SYSTEMS)
  const [valueSets, setValueSets] = useState<ValueSet[]>(VALUE_SETS)
  const published = useMemo(() => new Map(VALUE_SETS.map((v) => [v.url, expand(SYSTEMS, v)])), [])
  const editValueSet = (url: string, f: (v: ValueSet) => ValueSet) => setValueSets((all) => all.map((v) => (v.url === url ? f(v) : v)))
  const editConcepts = (url: string, concepts: Concept[]) => setSystems((all) => all.map((s) => (s.url === url ? { ...s, concepts, total: concepts.length } : s)))
  return { systems, valueSets, published, editValueSet, editConcepts }
}
export type Model = ReturnType<typeof useTerminology>

/** What a ValueSet change does to the forms that bind it, against its published expansion. */
export function Reach({ model, vs }: { model: Model; vs: ValueSet }) {
  const now = expand(model.systems, vs)
  const before = model.published.get(vs.url) ?? []
  const key = (x: { system: string; code: string }) => `${x.system}|${x.code}`
  const added = now.filter((x) => !before.some((b) => key(b) === key(x)))
  const removed = before.filter((x) => !now.some((b) => key(b) === key(x)))
  const forms = FORMS.filter((f) => f.valueSet === vs.url)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
      <span>
        <strong>{now.length}</strong> codes · against published {vs.version}: <span className={ok}>+{added.length}</span> <span className={removed.length ? bad : undefined}>−{removed.length}</span>
      </span>
      {removed.length ? <span className={bad}>No longer offered: {removed.map((r) => r.display).join(', ')}. Records already holding them stay valid against the version that issued them.</span> : null}
      <span className={muted}>Bound by: {forms.length ? forms.map((f) => `${f.title} → ${f.item}`).join('; ') : 'no form yet'}</span>
    </div>
  )
}

function Expansion({ model, vs, limit = 40 }: { model: Model; vs: ValueSet; limit?: number }) {
  const rows = expand(model.systems, vs)
  return (
    <table className={css({ width: '100%', fontSize: 'sm', '& td': { px: '2', py: '0.5', borderBottom: '1px solid', borderColor: 'border' } })}>
      <tbody>
        {rows.slice(0, limit).map((r) => (
          <tr key={`${r.system}|${r.code}`}>
            <td className={mono}>{r.code}</td>
            <td>{r.display}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

// --- Variant A: browse the code system, add to the open ValueSet -------------------------------

function VariantA({ model }: { model: Model }) {
  const [system, setSystem] = useState(SYSTEMS[0]?.url ?? '')
  const [query, setQuery] = useState('')
  const [open, setOpen] = useState(new Set(['01', '1F4', '1B1']))
  const [picked, setPicked] = useState<string | null>('1F40')
  const [basket, setBasket] = useState(VALUE_SETS[0]?.url ?? '')
  const cs = systemOf(model.systems, system)
  const vs = model.valueSets.find((v) => v.url === basket)
  const inBasket = new Set(vs ? expand(model.systems, vs).filter((x) => x.system === system).map((x) => x.code) : [])
  const concept = cs?.concepts.find((c) => c.code === picked)
  const add = (rule: Rule) => vs && model.editValueSet(vs.url, (v) => ({ ...v, include: [...v.include, rule] }))
  const exclude = (code: string) => vs && model.editValueSet(vs.url, (v) => ({ ...v, exclude: [...v.exclude, { system, codes: [code] }] }))
  const matches = (c: Concept) => query === '' || `${c.code} ${c.display}`.toLowerCase().includes(query.toLowerCase())
  const tree = (parent: string | undefined, depth: number): React.ReactNode =>
    cs?.concepts
      .filter((c) => c.parent === parent && (query === '' || matches(c) || cs.concepts.some((d) => d.parent === c.code)))
      .map((c) => {
        const kids = cs.concepts.some((d) => d.parent === c.code)
        const expanded = open.has(c.code) || query !== ''
        return (
          <li key={c.code}>
            <div className={cx(css({ display: 'flex', gap: '2', alignItems: 'center', py: '0.5', fontSize: 'sm' }), picked === c.code ? css({ bg: 'bg.selected' }) : undefined)} style={{ paddingLeft: `${depth * 1.1}rem` }}>
              <button type="button" aria-label={expanded ? 'Collapse' : 'Expand'} className={css({ width: '4', cursor: 'pointer' })} onClick={() => setOpen((o) => new Set(o.has(c.code) ? [...o].filter((x) => x !== c.code) : [...o, c.code]))}>
                {kids ? (expanded ? '▾' : '▸') : ''}
              </button>
              <button type="button" className={css({ textAlign: 'left', cursor: 'pointer', flex: '1' })} onClick={() => setPicked(c.code)}>
                <span className={mono}>{c.code}</span> {c.display}
              </button>
              {vs ? (inBasket.has(c.code) ? <span className={ok}>✓</span> : null) : null}
            </div>
            {kids && expanded ? <ul>{tree(c.code, depth + 1)}</ul> : null}
          </li>
        )
      })
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '14rem 1fr 22rem' }, gap: '3', alignItems: 'start' })}>
      <nav aria-label="Code systems" className={cx(box, css({ p: '2' }))}>
        <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          {model.systems.map((s) => (
            <li key={s.url}>
              <button type="button" onClick={() => setSystem(s.url)} className={cx(css({ width: '100%', textAlign: 'left', p: '2', borderRadius: 'sm', cursor: 'pointer', fontSize: 'sm' }), system === s.url ? css({ bg: 'bg.selected' }) : undefined)}>
                {s.title}
                <div className={muted}>
                  {s.local ? 'local' : 'external, read-only'} · {s.version} · {s.total.toLocaleString()} concepts
                </div>
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <input className={input} placeholder={`Search ${cs?.total.toLocaleString() ?? ''} concepts by code or words (the server searches; this sample holds a few)`} value={query} onChange={(e) => setQuery(e.target.value)} />
        <ul className={css({ maxHeight: '28rem', overflowY: 'auto' })}>{tree(undefined, 0)}</ul>
        {concept ? (
          <div className={cx(box, css({ p: '2', bg: 'bg.subtle', display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' }))}>
            <strong>
              <span className={mono}>{concept.code}</span> {concept.display}
            </strong>
            {concept.definition ? <span>{concept.definition}</span> : null}
            <span className={muted}>In ValueSets: {model.valueSets.filter((v) => expand(model.systems, v).some((x) => x.system === system && x.code === concept.code)).map((v) => v.title).join(', ') || 'none'}</span>
            {vs ? (
              <div className={css({ display: 'flex', gap: '2', flexWrap: 'wrap' })}>
                <button type="button" className={small} onClick={() => add({ system, codes: [concept.code] })}>
                  Add this code
                </button>
                <button type="button" className={small} onClick={() => add({ system, codes: [], filter: { op: 'is-a', value: concept.code } })}>
                  Add it and everything under it
                </button>
                <button type="button" className={small} disabled={!inBasket.has(concept.code)} onClick={() => exclude(concept.code)}>
                  Exclude it
                </button>
              </div>
            ) : null}
          </div>
        ) : null}
      </section>
      <aside className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <label className={css({ fontSize: 'sm' })}>
          Adding to
          <select className={input} value={basket} onChange={(e) => setBasket(e.target.value)}>
            {model.valueSets
              .filter((v) => v.where === 'draft')
              .map((v) => (
                <option key={v.url} value={v.url}>
                  {v.title} (draft)
                </option>
              ))}
          </select>
        </label>
        {vs ? (
          <>
            <Reach model={model} vs={vs} />
            <div className={css({ maxHeight: '22rem', overflowY: 'auto' })}>
              <Expansion model={model} vs={vs} />
            </div>
          </>
        ) : null}
      </aside>
    </div>
  )
}

// --- Variant B: the ValueSet as rules ----------------------------------------------------------

function RuleCard({ model, rule, onChange, onRemove }: { model: Model; rule: Rule; onChange: (r: Rule) => void; onRemove: () => void }) {
  const cs = systemOf(model.systems, rule.system)
  const [adding, setAdding] = useState('')
  const count = ruleCodes(model.systems, rule).length
  const found = adding.length > 1 ? (cs?.concepts ?? []).filter((c) => `${c.code} ${c.display}`.toLowerCase().includes(adding.toLowerCase())).slice(0, 6) : []
  const mode = rule.filter ? 'filter' : rule.codes.length ? 'codes' : 'all'
  return (
    <div className={cx(box, css({ p: '2', display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' }))}>
      <div className={css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap' })}>
        <strong>{cs?.title ?? rule.system}</strong>
        <select
          aria-label="Which codes"
          className={cx(input, css({ width: 'auto' }))}
          value={mode}
          onChange={(e) => onChange(e.target.value === 'filter' ? { ...rule, codes: [], filter: { op: 'is-a', value: cs?.concepts[0]?.code ?? '' } } : e.target.value === 'all' ? { ...rule, codes: [], filter: undefined } : { ...rule, filter: undefined, codes: [] })}
        >
          <option value="codes">these codes</option>
          <option value="filter">a concept and what is under it</option>
          <option value="all" disabled={cs !== undefined && !cs.local}>
            the whole code system{cs !== undefined && !cs.local ? ' (too large)' : ''}
          </option>
        </select>
        <span className={muted}>{count} codes</span>
        <button type="button" className={cx(small, css({ ml: 'auto' }))} onClick={onRemove}>
          Remove rule
        </button>
      </div>
      {rule.filter ? (
        <div className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
          <select className={cx(input, css({ width: 'auto' }))} value={rule.filter.op} onChange={(e) => rule.filter && onChange({ ...rule, filter: { ...rule.filter, op: e.target.value as 'is-a' } })}>
            <option value="is-a">is-a (it and everything under it)</option>
            <option value="descendent-of">descendent-of (only what is under it)</option>
          </select>
          <select className={cx(input, css({ width: 'auto' }))} value={rule.filter.value} onChange={(e) => rule.filter && onChange({ ...rule, filter: { ...rule.filter, value: e.target.value } })}>
            {cs?.concepts
              .filter((c) => cs.concepts.some((d) => d.parent === c.code))
              .map((c) => (
                <option key={c.code} value={c.code}>
                  {c.code} {c.display}
                </option>
              ))}
          </select>
        </div>
      ) : null}
      {mode === 'codes' ? (
        <>
          <div className={css({ display: 'flex', gap: '1', flexWrap: 'wrap' })}>
            {rule.codes.map((c) => (
              <span key={c} className={cx(box, css({ px: '2', py: '0.5', fontSize: 'xs', display: 'inline-flex', gap: '1' }))}>
                {c} {cs?.concepts.find((x) => x.code === c)?.display}
                <button type="button" aria-label={`Remove ${c}`} className={css({ cursor: 'pointer' })} onClick={() => onChange({ ...rule, codes: rule.codes.filter((x) => x !== c) })}>
                  ✕
                </button>
              </span>
            ))}
          </div>
          <input className={input} placeholder="Find a code to add" value={adding} onChange={(e) => setAdding(e.target.value)} />
          {found.map((c) => (
            <button key={c.code} type="button" className={cx(small, css({ textAlign: 'left' }))} onClick={() => (onChange({ ...rule, codes: [...rule.codes, c.code] }), setAdding(''))}>
              + {c.code} {c.display}
            </button>
          ))}
        </>
      ) : null}
    </div>
  )
}

function VariantB({ model }: { model: Model }) {
  const [url, setUrl] = useState(VALUE_SETS[0]?.url ?? '')
  const vs = model.valueSets.find((v) => v.url === url)
  if (!vs) return null
  const set = (part: 'include' | 'exclude', rules: Rule[]) => model.editValueSet(vs.url, (v) => ({ ...v, [part]: rules }))
  const section = (part: 'include' | 'exclude') => (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <h2 className={h2}>{part === 'include' ? 'Include' : 'Then leave out'}</h2>
      {vs[part].map((r, n) => (
        <RuleCard key={n} model={model} rule={r} onChange={(next) => set(part, vs[part].map((x, m) => (m === n ? next : x)))} onRemove={() => set(part, vs[part].filter((_, m) => m !== n))} />
      ))}
      <select aria-label={`Add a rule to ${part}`} className={cx(input, css({ width: 'auto' }))} value="" onChange={(e) => e.target.value && set(part, [...vs[part], { system: e.target.value, codes: [] }])}>
        <option value="">+ Add a rule from…</option>
        {model.systems.map((s) => (
          <option key={s.url} value={s.url}>
            {s.title}
          </option>
        ))}
      </select>
    </section>
  )
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '16rem 1fr' }, gap: '3', alignItems: 'start' })}>
      <nav aria-label="ValueSets" className={cx(box, css({ p: '2' }))}>
        {model.valueSets.map((v) => (
          <button key={v.url} type="button" onClick={() => setUrl(v.url)} className={cx(css({ width: '100%', textAlign: 'left', p: '2', borderRadius: 'sm', cursor: 'pointer', fontSize: 'sm' }), url === v.url ? css({ bg: 'bg.selected' }) : undefined)}>
            {v.title}
            <div className={muted}>
              {v.where} · {v.version} · {expand(model.systems, v).length} codes
            </div>
          </button>
        ))}
      </nav>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        <div className={cx(box, css({ p: '3' }))}>
          <h2 className={h2}>{vs.title}</h2>
          <span className={mono}>
            {vs.url}|{vs.version}
          </span>
          <Reach model={model} vs={vs} />
        </div>
        {section('include')}
        {section('exclude')}
        <section className={cx(box, css({ p: '3' }))}>
          <h2 className={h2}>Expansion</h2>
          <p className={muted}>What the terminology service will return, and what publication pins in the manifest.</p>
          <Expansion model={model} vs={vs} />
        </section>
      </div>
    </div>
  )
}

// --- Variant C: a local CodeSystem as a sheet ----------------------------------------------------

export function VariantC({ model }: { model: Model }) {
  const locals = model.systems.filter((s) => s.local)
  const [url, setUrl] = useState(locals[0]?.url ?? '')
  const [paste, setPaste] = useState('')
  const cs = systemOf(model.systems, url)
  const original = SYSTEMS.find((s) => s.url === url)
  if (!cs || !original) return null
  const change = bump(original.concepts, cs.concepts)
  const edit = (n: number, patch: Partial<Concept>) => model.editConcepts(cs.url, cs.concepts.map((c, m) => (m === n ? { ...c, ...patch } : c)))
  const cell = css({ px: '1', py: '1', borderBottom: '1px solid', borderColor: 'border', verticalAlign: 'top' })
  const plain = cx(input, css({ minHeight: '1.8rem', fontSize: 'sm' }))
  const reached = model.valueSets.filter((v) => v.include.some((r) => r.system === cs.url))
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 22rem' }, gap: '3', alignItems: 'start' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <div className={css({ display: 'flex', gap: '2', flexWrap: 'wrap', alignItems: 'center' })}>
          {locals.map((s) => (
            <button key={s.url} type="button" aria-pressed={url === s.url} className={cx(ghost, url === s.url ? css({ bg: 'bg.selected', borderColor: 'accent.default' }) : undefined)} onClick={() => setUrl(s.url)}>
              {s.title} ({s.where})
            </button>
          ))}
          <span className={muted}>External code systems (ICD-11, LOINC) arrive as terminology packages and are read-only here.</span>
        </div>
        <div className={cx(box, css({ overflowX: 'auto' }))}>
          <table className={css({ width: '100%', borderCollapse: 'collapse', fontSize: 'sm' })}>
            <thead>
              <tr>
                {['Code', 'Display', 'Definition', 'Under', 'Retired'].map((h) => (
                  <th key={h} className={cx(cell, css({ textAlign: 'left' }))}>
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {cs.concepts.map((c, n) => {
                const isNew = !original.concepts.some((o) => o.code === c.code)
                return (
                  <tr key={n} className={c.retired ? css({ opacity: 0.6 }) : undefined}>
                    <td className={cell}>
                      <input aria-label="Code" className={cx(plain, mono)} value={c.code} readOnly={!isNew} title={isNew ? undefined : 'A published code never changes'} onChange={(e) => edit(n, { code: e.target.value })} />
                    </td>
                    <td className={cell}>
                      <input aria-label="Display" className={plain} value={c.display} onChange={(e) => edit(n, { display: e.target.value })} />
                    </td>
                    <td className={cell}>
                      <input aria-label="Definition" className={plain} value={c.definition ?? ''} onChange={(e) => edit(n, { definition: e.target.value })} />
                    </td>
                    <td className={cell}>
                      <select aria-label="Under" className={plain} value={c.parent ?? ''} onChange={(e) => edit(n, { parent: e.target.value || undefined })}>
                        <option value="">—</option>
                        {cs.concepts
                          .filter((x) => x.code !== c.code)
                          .map((x) => (
                            <option key={x.code} value={x.code}>
                              {x.code}
                            </option>
                          ))}
                      </select>
                    </td>
                    <td className={cell}>
                      {isNew ? (
                        <button type="button" className={small} onClick={() => model.editConcepts(cs.url, cs.concepts.filter((_, m) => m !== n))}>
                          Delete
                        </button>
                      ) : (
                        <input type="checkbox" aria-label="Retired" checked={c.retired === true} onChange={(e) => edit(n, { retired: e.target.checked })} />
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
          <div className={css({ display: 'flex', gap: '2', p: '2', flexWrap: 'wrap', alignItems: 'flex-start' })}>
            <button type="button" className={ghost} onClick={() => model.editConcepts(cs.url, [...cs.concepts, { code: '', display: '' }])}>
              Add a code
            </button>
            <textarea className={cx(input, mono, css({ flex: '1', minHeight: '3rem', py: '1' }))} placeholder={'Paste rows: code<TAB>display'} value={paste} onChange={(e) => setPaste(e.target.value)} />
            <button
              type="button"
              className={ghost}
              disabled={paste.trim() === ''}
              onClick={() => {
                const rows = paste
                  .split('\n')
                  .map((l) => l.split('\t'))
                  .filter((p) => p[0])
                  .map(([code = '', display = '']) => ({ code: code.trim(), display: display.trim() }))
                model.editConcepts(cs.url, [...cs.concepts, ...rows])
                setPaste('')
              }}
            >
              Add pasted rows
            </button>
          </div>
        </div>
      </div>
      <aside className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' }))}>
          <h2 className={h2}>Version</h2>
          <span>
            {original.version} → <strong>{nextVersion(original.version, change.level)}</strong> ({change.level === 'none' ? 'no change' : change.level})
          </span>
          <ul className={css({ pl: '4', listStyle: 'disc' })}>
            {change.why.map((w) => (
              <li key={w} className={/removed|redefined/.test(w) ? bad : undefined}>
                {w}
              </li>
            ))}
          </ul>
          <span className={muted}>Removing or redefining a code is major: retire it instead, so records that hold it still resolve.</span>
        </section>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
          <h2 className={h2}>What it reaches</h2>
          {reached.map((v) => (
            <div key={v.url}>
              <strong className={css({ fontSize: 'sm' })}>{v.title}</strong>
              <Reach model={model} vs={v} />
            </div>
          ))}
        </section>
      </aside>
    </div>
  )
}

// --- The form designer's picker (shown under every variant) ---------------------------------------

function Picker({ model }: { model: Model }) {
  const [query, setQuery] = useState('')
  const [chosen, setChosen] = useState<string | null>(null)
  const [bound, setBound] = useState<string | null>(null)
  const list = model.valueSets.filter((v) => `${v.title} ${v.url}`.toLowerCase().includes(query.toLowerCase()))
  const vs = model.valueSets.find((v) => v.url === chosen)
  const count = vs ? expand(model.systems, vs).length : 0
  return (
    <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
      <h2 className={h2}>In the form designer: bind “Diagnosis” to a ValueSet</h2>
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 1fr' }, gap: '3' })}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          <input className={input} placeholder="Search ValueSets" value={query} onChange={(e) => setQuery(e.target.value)} />
          {list.map((v) => (
            <button key={v.url} type="button" onClick={() => setChosen(v.url)} className={cx(css({ textAlign: 'left', p: '2', borderRadius: 'sm', cursor: 'pointer', fontSize: 'sm' }), chosen === v.url ? css({ bg: 'bg.selected' }) : undefined)}>
              {v.title} <span className={muted}>{v.where === 'draft' ? `draft in this package, ${v.version}` : `published ${v.version}`}</span>
            </button>
          ))}
        </div>
        {vs ? (
          <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' })}>
            <span>
              {count} codes · suggested widget: <strong>{count <= 7 ? 'radio buttons' : count <= 30 ? 'drop-down' : 'search-as-you-type'}</strong>
            </span>
            <div className={css({ maxHeight: '12rem', overflowY: 'auto' })}>
              <Expansion model={model} vs={vs} limit={10} />
            </div>
            <button type="button" className={ghost} onClick={() => setBound(`${vs.url}|${vs.version}`)}>
              Bind {vs.title}
            </button>
            {bound ? <span className={ok}>Bound to {bound}. Publication pins its expansion in the manifest.</span> : null}
          </div>
        ) : (
          <p className={muted}>Choose a ValueSet to preview its codes.</p>
        )}
      </div>
    </section>
  )
}

// --- The shell -----------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'Browse the code system, add to the open ValueSet', View: VariantA },
  { key: 'B', name: 'The ValueSet as rules with counts', View: VariantB },
  { key: 'C', name: 'A local CodeSystem as a sheet, with its version bump', View: VariantC },
] as const

export function TerminologyPrototype() {
  const model = useTerminology()
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
        <p className={muted}>← ug.moh.opd-terminology 3.2.0, draft · Terminology</p>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>Code systems and ValueSets</h1>
      </header>
      <View key={current.key} model={model} />
      <Picker model={model} />
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
