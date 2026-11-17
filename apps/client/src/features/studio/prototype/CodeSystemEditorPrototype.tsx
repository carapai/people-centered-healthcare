// PROTOTYPE (#519), throwaway: three ways to edit a local CodeSystem's concepts on
// /prototype/codesystem-editor, switched with ?variant=A|B|C and the floating bar (← / → keys).
// A: in the chosen terminology browser: the tree gets "+ code under it", the concept panel
//    becomes a form, and the version bump and what the change reaches sit beside it.
// B: the whole code list as a sheet (the earlier terminology variant C).
// C: a list of changes (add, change display, redefine, move, retire), each with a reason,
//    from which the new version and its bump are worked out.
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import { Reach, VariantC as Sheet, useTerminology, type Model } from './TerminologyPrototype'
import { SYSTEMS, bump, nextVersion, type Concept } from './terminologyData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', width: '100%', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const small = cx(ghost, css({ minHeight: 'auto', py: '0', px: '2', fontSize: 'xs' }))
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs' })
const bad = css({ color: 'danger.text' })
const ok = css({ color: 'success.text' })

function Version({ model, url }: { model: Model; url: string }) {
  const cs = model.systems.find((s) => s.url === url)
  const original = SYSTEMS.find((s) => s.url === url)
  if (!cs || !original) return null
  const change = bump(original.concepts, cs.concepts)
  return (
    <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' }))}>
      <h2 className={h2}>Version</h2>
      <span>
        {original.version} → <strong>{nextVersion(original.version, change.level)}</strong> ({change.level === 'none' ? 'no change yet' : change.level})
      </span>
      <ul className={css({ pl: '4', listStyle: 'disc' })}>
        {change.why.map((w) => (
          <li key={w} className={/removed|redefined/.test(w) ? bad : undefined}>
            {w}
          </li>
        ))}
      </ul>
      <span className={muted}>Adding a code is minor, a display change a patch; removing or redefining one is major, so retire instead.</span>
    </section>
  )
}

function Reaches({ model, url }: { model: Model; url: string }) {
  const reached = model.valueSets.filter((v) => v.include.some((r) => r.system === url))
  return (
    <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
      <h2 className={h2}>What it reaches</h2>
      {reached.length ? null : <span className={muted}>No ValueSet uses it yet.</span>}
      {reached.map((v) => (
        <div key={v.url}>
          <strong className={css({ fontSize: 'sm' })}>{v.title}</strong>
          <Reach model={model} vs={v} />
        </div>
      ))}
    </section>
  )
}

// --- Variant A: edit in the browser ------------------------------------------------------------

function VariantA({ model }: { model: Model }) {
  const locals = model.systems.filter((s) => s.local)
  const [url, setUrl] = useState(locals[1]?.url ?? locals[0]?.url ?? '')
  const [picked, setPicked] = useState<string | null>(null)
  const cs = model.systems.find((s) => s.url === url)
  const original = SYSTEMS.find((s) => s.url === url)
  if (!cs || !original) return null
  const concept = cs.concepts.find((c) => c.code === picked)
  const isNew = (code: string) => !original.concepts.some((c) => c.code === code)
  const set = (code: string, patch: Partial<Concept>) => model.editConcepts(cs.url, cs.concepts.map((c) => (c.code === code ? { ...c, ...patch } : c)))
  const addUnder = (parent: string | undefined) => {
    const code = `new-${cs.concepts.length + 1}`
    model.editConcepts(cs.url, [...cs.concepts, { code, display: 'New code', parent }])
    setPicked(code)
  }
  const tree = (parent: string | undefined, depth: number): React.ReactNode =>
    cs.concepts
      .filter((c) => c.parent === parent)
      .map((c) => (
        <li key={c.code}>
          <div className={cx(css({ display: 'flex', gap: '2', alignItems: 'center', py: '0.5', fontSize: 'sm' }), picked === c.code ? css({ bg: 'bg.selected' }) : undefined)} style={{ paddingLeft: `${depth * 1.1}rem` }}>
            <button type="button" className={cx(css({ textAlign: 'left', cursor: 'pointer', flex: '1' }), c.retired ? css({ textDecoration: 'line-through', color: 'fg.muted' }) : undefined)} onClick={() => setPicked(c.code)}>
              <span className={mono}>{c.code}</span> {c.display}
              {isNew(c.code) ? <span className={ok}> new</span> : null}
              {c.retired ? <span className={muted}> retired</span> : null}
            </button>
            <button type="button" className={small} aria-label={`Add a code under ${c.code}`} onClick={() => addUnder(c.code)}>
              + under
            </button>
          </div>
          <ul>{tree(c.code, depth + 1)}</ul>
        </li>
      ))
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '14rem 1fr 22rem' }, gap: '3', alignItems: 'start' })}>
      <nav aria-label="Code systems" className={cx(box, css({ p: '2' }))}>
        {model.systems.map((s) => (
          <button key={s.url} type="button" disabled={!s.local} onClick={() => (setUrl(s.url), setPicked(null))} className={cx(css({ width: '100%', textAlign: 'left', p: '2', borderRadius: 'sm', cursor: 'pointer', fontSize: 'sm', _disabled: { cursor: 'default', opacity: 0.6 } }), url === s.url ? css({ bg: 'bg.selected' }) : undefined)}>
            {s.title}
            <div className={muted}>{s.local ? `local · ${s.where} · ${s.version}` : 'external, read-only'}</div>
          </button>
        ))}
      </nav>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '3' }))}>
        <div className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' })}>
          <h2 className={h2}>{cs.title}</h2>
          <button type="button" className={ghost} onClick={() => addUnder(undefined)}>
            Add a code
          </button>
        </div>
        <ul>{tree(undefined, 0)}</ul>
        {concept ? (
          <div className={cx(box, css({ p: '3', bg: 'bg.subtle', display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' }))}>
            <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              Code {isNew(concept.code) ? <span className={muted}>Choose it carefully: once published it never changes</span> : <span className={muted}>Published: fixed</span>}
              <input className={cx(input, mono)} value={concept.code} readOnly={!isNew(concept.code)} onChange={(e) => (set(concept.code, { code: e.target.value }), setPicked(e.target.value))} />
            </label>
            <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              Display <span className={muted}>What people read. Changing it is a patch.</span>
              <input className={input} value={concept.display} onChange={(e) => set(concept.code, { display: e.target.value })} />
            </label>
            <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              Definition <span className={muted}>What it means. Changing the meaning of a published code is major.</span>
              <textarea className={cx(input, css({ minHeight: '3rem', py: '1' }))} value={concept.definition ?? ''} onChange={(e) => set(concept.code, { definition: e.target.value })} />
            </label>
            <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              Under
              <select className={input} value={concept.parent ?? ''} onChange={(e) => set(concept.code, { parent: e.target.value || undefined })}>
                <option value="">(top level)</option>
                {cs.concepts
                  .filter((c) => c.code !== concept.code)
                  .map((c) => (
                    <option key={c.code} value={c.code}>
                      {c.code} {c.display}
                    </option>
                  ))}
              </select>
            </label>
            {isNew(concept.code) ? (
              <button type="button" className={cx(ghost, bad)} onClick={() => (model.editConcepts(cs.url, cs.concepts.filter((c) => c.code !== concept.code)), setPicked(null))}>
                Delete (not yet published)
              </button>
            ) : (
              <label>
                <input type="checkbox" checked={concept.retired === true} onChange={(e) => set(concept.code, { retired: e.target.checked })} /> Retired: kept so records that hold it still resolve, no longer offered on forms
              </label>
            )}
          </div>
        ) : (
          <p className={muted}>Choose a code to edit it, or add one.</p>
        )}
      </section>
      <aside className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <Version model={model} url={cs.url} />
        <Reaches model={model} url={cs.url} />
      </aside>
    </div>
  )
}

// --- Variant C: a list of changes ---------------------------------------------------------------

type Change = { kind: 'add' | 'display' | 'definition' | 'move' | 'retire'; code: string; value: string; reason: string }

function VariantC({ model }: { model: Model }) {
  const locals = model.systems.filter((s) => s.local)
  const [url, setUrl] = useState(locals[1]?.url ?? locals[0]?.url ?? '')
  const [changes, setChanges] = useState<Change[]>([])
  const [draft, setDraft] = useState<Change>({ kind: 'add', code: '', value: '', reason: '' })
  const original = SYSTEMS.find((s) => s.url === url)
  useEffect(() => {
    if (!original) return
    let concepts: Concept[] = original.concepts.map((c) => ({ ...c }))
    for (const ch of changes) {
      if (ch.kind === 'add') concepts = [...concepts, { code: ch.code, display: ch.value }]
      else concepts = concepts.map((c) => (c.code !== ch.code ? c : ch.kind === 'display' ? { ...c, display: ch.value } : ch.kind === 'definition' ? { ...c, definition: ch.value } : ch.kind === 'move' ? { ...c, parent: ch.value || undefined } : { ...c, retired: true }))
    }
    model.editConcepts(url, concepts)
  }, [changes, url]) // eslint-disable-line react-hooks/exhaustive-deps
  if (!original) return null
  const labels: Record<Change['kind'], [string, string]> = { add: ['Add a code', 'Display'], display: ['Change a display', 'New display'], definition: ['Redefine a code', 'New definition'], move: ['Move a code under another', 'New parent code (blank: top level)'], retire: ['Retire a code', ''] }
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 22rem' }, gap: '3', alignItems: 'start' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <div className={css({ display: 'flex', gap: '2', flexWrap: 'wrap' })}>
          {locals.map((s) => (
            <button key={s.url} type="button" aria-pressed={url === s.url} className={cx(ghost, url === s.url ? css({ bg: 'bg.selected', borderColor: 'accent.default' }) : undefined)} onClick={() => (setUrl(s.url), setChanges([]))}>
              {s.title}
            </button>
          ))}
        </div>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
          <h2 className={h2}>Changes in this draft</h2>
          {changes.length === 0 ? <p className={muted}>No changes yet. Each change carries a reason, which reviewers read in the discussion.</p> : null}
          <ol className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
            {changes.map((ch, n) => (
              <li key={n} className={cx(box, css({ p: '2', display: 'flex', gap: '2', alignItems: 'baseline' }))}>
                <strong>{n + 1}.</strong>
                <span className={css({ flex: '1' })}>
                  {labels[ch.kind][0]} <span className={mono}>{ch.code}</span>
                  {ch.value ? <> → “{ch.value}”</> : null}
                  <div className={muted}>Why: {ch.reason || '(no reason given)'}</div>
                </span>
                <button type="button" className={small} onClick={() => setChanges(changes.filter((_, m) => m !== n))}>
                  Undo
                </button>
              </li>
            ))}
          </ol>
          <div className={cx(box, css({ p: '2', display: 'grid', gridTemplateColumns: { base: '1fr', md: '12rem 10rem 1fr' }, gap: '2', alignItems: 'end', fontSize: 'sm' }))}>
            <label>
              Change
              <select className={input} value={draft.kind} onChange={(e) => setDraft({ ...draft, kind: e.target.value as Change['kind'] })}>
                {Object.entries(labels).map(([k, [l]]) => (
                  <option key={k} value={k}>
                    {l}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Code
              {draft.kind === 'add' ? (
                <input className={cx(input, mono)} value={draft.code} onChange={(e) => setDraft({ ...draft, code: e.target.value })} />
              ) : (
                <select className={input} value={draft.code} onChange={(e) => setDraft({ ...draft, code: e.target.value })}>
                  <option value="">—</option>
                  {model.systems
                    .find((s) => s.url === url)
                    ?.concepts.map((c) => (
                      <option key={c.code} value={c.code}>
                        {c.code}
                      </option>
                    ))}
                </select>
              )}
            </label>
            {labels[draft.kind][1] ? (
              <label>
                {labels[draft.kind][1]}
                <input className={input} value={draft.value} onChange={(e) => setDraft({ ...draft, value: e.target.value })} />
              </label>
            ) : (
              <span />
            )}
            <label className={css({ gridColumn: { md: '1 / 3' } })}>
              Why
              <input className={input} value={draft.reason} placeholder="e.g. IMCI 2024 adds this sign" onChange={(e) => setDraft({ ...draft, reason: e.target.value })} />
            </label>
            <button type="button" className={ghost} disabled={!draft.code} onClick={() => (setChanges([...changes, draft]), setDraft({ kind: draft.kind, code: '', value: '', reason: '' }))}>
              Add change
            </button>
          </div>
        </section>
        <section className={cx(box, css({ p: '3' }))}>
          <h2 className={h2}>The code list after these changes</h2>
          <table className={css({ width: '100%', fontSize: 'sm', '& td': { px: '2', py: '0.5', borderBottom: '1px solid', borderColor: 'border' } })}>
            <tbody>
              {model.systems
                .find((s) => s.url === url)
                ?.concepts.map((c) => (
                  <tr key={c.code} className={c.retired ? css({ color: 'fg.muted', textDecoration: 'line-through' }) : undefined}>
                    <td className={mono}>{c.code}</td>
                    <td>{c.display}</td>
                    <td className={muted}>{c.parent ? `under ${c.parent}` : ''}</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </section>
      </div>
      <aside className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <Version model={model} url={url} />
        <Reaches model={model} url={url} />
      </aside>
    </div>
  )
}

// --- The shell -----------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'Edit in the terminology browser', View: VariantA },
  { key: 'B', name: 'The whole code list as a sheet', View: Sheet },
  { key: 'C', name: 'A list of changes, each with a reason', View: VariantC },
] as const

export function CodeSystemEditorPrototype() {
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
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>Edit a local code list</h1>
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
