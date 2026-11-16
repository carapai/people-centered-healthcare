// PROTOTYPE (#478), throwaway: three structurally different package workspaces for the Studio,
// on /prototype/studio-workspace, switched with ?variant=A|B|C and the floating bar (← / → keys).
// A: a lifecycle board. B: a three-pane workbench. C: a list, then each draft as one review page.
// Read-only; the buttons only show whether the signed-in person may act, and why not (ADR 0025).
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import { PACKAGES, PEOPLE, STAGES, actions, approversNeeded, person, type Pkg } from './data'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const chip = css({ display: 'inline-block', px: '2', py: '0.5', borderRadius: 'full', fontSize: 'xs', border: '1px solid', borderColor: 'border' })
const btn = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', bg: 'accent.default', color: 'accent.fg', _disabled: { opacity: '0.45', cursor: 'not-allowed' } })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface' })

const stageLabel = (s: Pkg['stage']) => STAGES.find((x) => x.key === s)?.label ?? s
const changeMark = { added: '+', changed: '~', removed: '−', unchanged: '·' } as const

function Me({ me, setMe }: { me: string; setMe: (id: string) => void }) {
  return (
    <label className={css({ display: 'flex', gap: '2', alignItems: 'center', fontSize: 'sm' })}>
      Signed in as
      <select value={me} onChange={(e) => setMe(e.target.value)} className={css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control' })}>
        {PEOPLE.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name} ({p.role})
          </option>
        ))}
      </select>
    </label>
  )
}

function Actions({ pkg, me, vertical }: { pkg: Pkg; me: string; vertical?: boolean }) {
  return (
    <div className={css({ display: 'flex', flexDirection: vertical ? 'column' : 'row', flexWrap: 'wrap', gap: '2' })}>
      {actions(pkg, me).map((a) => (
        <div key={a.label} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          <button type="button" className={btn} disabled={!a.allowed}>
            {a.label}
          </button>
          {a.allowed || a.why === undefined ? null : <span className={muted}>{a.why}</span>}
        </div>
      ))}
    </div>
  )
}

function Checks({ pkg }: { pkg: Pkg }) {
  return (
    <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      {pkg.checks.map((c) => (
        <li key={c.kind + c.name} className={css({ display: 'flex', gap: '2' })}>
          <span aria-hidden className={css({ color: c.ok ? 'success.text' : 'danger.text', fontWeight: 'bold' })}>
            {c.ok ? '✓' : '✗'}
          </span>
          <span>
            <strong>{c.kind === 'simulation' ? `Scenario ${c.name}` : c.name}</strong> <span className={muted}>{c.detail}</span>
          </span>
        </li>
      ))}
    </ul>
  )
}

function Approvals({ pkg }: { pkg: Pkg }) {
  const need = approversNeeded(pkg.kind)
  return (
    <p className={muted}>
      {pkg.kind} package: {need} approver{need > 1 ? 's' : ''}, neither the author. Approved by {pkg.approvals.length === 0 ? 'nobody yet' : pkg.approvals.map((a) => person(a)?.name).join(', ')}.
    </p>
  )
}

function History({ pkg }: { pkg: Pkg }) {
  return (
    <ol className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      {pkg.history.map((e) => (
        <li key={e.at + e.what}>
          <span className={muted}>{e.at}</span> <strong>{person(e.who)?.name}</strong> {e.what}
          {e.comment === undefined ? null : <blockquote className={css({ borderLeft: '3px solid', borderColor: 'border', pl: '2', mt: '1' })}>{e.comment}</blockquote>}
        </li>
      ))}
    </ol>
  )
}

// A: the lifecycle board. Columns are the stages; a card opens a drawer with everything.
function VariantA({ me }: { me: string }) {
  const [open, setOpen] = useState<Pkg | undefined>()
  return (
    <div className={css({ display: 'flex', gap: '3', overflowX: 'auto', pb: '16' })}>
      {STAGES.map((s) => (
        <section key={s.key} className={css({ minWidth: '15rem', flex: '1', display: 'flex', flexDirection: 'column', gap: '2' })}>
          <h2 className={css({ fontWeight: 'semibold' })}>
            {s.label} <span className={muted}>({PACKAGES.filter((p) => p.stage === s.key).length})</span>
          </h2>
          {PACKAGES.filter((p) => p.stage === s.key).map((p) => {
            const waitingOnMe = actions(p, me).some((a) => a.allowed && ['Approve', 'Request changes', 'Submit for review', 'Publish'].includes(a.label))
            return (
              <button key={p.name} type="button" onClick={() => setOpen(p)} className={cx(box, css({ p: '3', textAlign: 'left', cursor: 'pointer', borderColor: waitingOnMe ? 'accent.default' : 'border', borderWidth: waitingOnMe ? '2px' : '1px' }))}>
                <div className={css({ fontWeight: 'semibold' })}>{p.title}</div>
                <div className={muted}>
                  {p.name} {p.version} · {p.kind}
                </div>
                <div className={muted}>by {person(p.author)?.name}</div>
                {p.checks.some((c) => !c.ok) ? <div className={css({ color: 'danger.text', fontSize: 'sm' })}>{p.checks.filter((c) => !c.ok).length} failing check</div> : null}
                {waitingOnMe ? <div className={css({ color: 'accent.text', fontSize: 'sm', fontWeight: 'semibold' })}>Waiting on you</div> : null}
              </button>
            )
          })}
        </section>
      ))}
      {open === undefined ? null : (
        <aside className={cx(box, css({ position: 'fixed', top: '0', right: '0', bottom: '0', width: 'min(36rem, 100vw)', p: '5', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '4', boxShadow: 'lg' }))}>
          <button type="button" className={ghost} onClick={() => setOpen(undefined)}>
            Close
          </button>
          <h2 className={css({ fontSize: 'xl', fontWeight: 'semibold' })}>
            {open.title} {open.version}
          </h2>
          <p className={muted}>
            {stageLabel(open.stage)} · {open.bump} change from {open.previous}
          </p>
          <Actions pkg={open} me={me} />
          <Approvals pkg={open} />
          <h3 className={css({ fontWeight: 'semibold' })}>Artefacts</h3>
          <ul>
            {open.artefacts.map((a) => (
              <li key={a.type + a.name}>
                {changeMark[a.change]} {a.type} <strong>{a.name}</strong> <span className={muted}>{a.summary}</span>
              </li>
            ))}
          </ul>
          <h3 className={css({ fontWeight: 'semibold' })}>Checks</h3>
          <Checks pkg={open} />
          <h3 className={css({ fontWeight: 'semibold' })}>History and comments</h3>
          <History pkg={open} />
        </aside>
      )}
    </div>
  )
}

// B: the workbench. Packages and artefacts in a tree, the artefact in the middle (with a diff
// tab), and the lifecycle in a fixed right rail.
function VariantB({ me }: { me: string }) {
  const [pkg, setPkg] = useState(PACKAGES[0] as Pkg)
  const [artefact, setArtefact] = useState(0)
  const [tab, setTab] = useState<'diff' | 'content'>('diff')
  const a = pkg.artefacts[artefact] ?? pkg.artefacts[0]
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: '16rem 1fr 20rem', gap: '3', minHeight: '70vh', pb: '16' })}>
      <nav aria-label="Packages" className={cx(box, css({ p: '2', overflowY: 'auto' }))}>
        {PACKAGES.map((p) => (
          <div key={p.name}>
            <button
              type="button"
              onClick={() => {
                setPkg(p)
                setArtefact(0)
              }}
              className={css({ width: '100%', textAlign: 'left', p: '1', borderRadius: 'sm', bg: p === pkg ? 'bg.selected' : 'transparent', cursor: 'pointer' })}
            >
              ▸ {p.name} <span className={muted}>{p.version}</span>
              <div className={muted}>{stageLabel(p.stage)}</div>
            </button>
            {p === pkg
              ? p.artefacts.map((x, i) => (
                  <button key={x.type + x.name} type="button" onClick={() => setArtefact(i)} className={css({ display: 'block', width: '100%', textAlign: 'left', pl: '5', py: '0.5', fontSize: 'sm', bg: i === artefact ? 'bg.hover' : 'transparent', cursor: 'pointer' })}>
                    {changeMark[x.change]} {x.name} <span className={muted}>{x.type}</span>
                  </button>
                ))
              : null}
          </div>
        ))}
      </nav>
      <section className={cx(box, css({ p: '4', display: 'flex', flexDirection: 'column', gap: '3' }))}>
        <header className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' })}>
          <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
            {a?.type} {a?.name}
          </h2>
          <div className={css({ display: 'flex', gap: '1' })}>
            {(['diff', 'content'] as const).map((t) => (
              <button key={t} type="button" className={ghost} aria-pressed={tab === t} onClick={() => setTab(t)}>
                {t === 'diff' ? `Changes since ${pkg.previous}` : 'Editor'}
              </button>
            ))}
          </div>
        </header>
        {tab === 'diff' ? (
          <pre className={css({ fontFamily: 'mono', fontSize: 'sm', bg: 'bg.subtle', p: '3', borderRadius: 'md', whiteSpace: 'pre-wrap' })}>
            {a?.change === 'unchanged' ? 'No changes.' : `${a?.change === 'added' ? '+ new artefact' : '~ changed'}\n\n${a?.summary}\n\nSemver: ${pkg.bump} (${pkg.previous} → ${pkg.version})`}
          </pre>
        ) : (
          <div className={cx(css({ p: '6', border: '2px dashed', borderColor: 'border', borderRadius: 'md', textAlign: 'center' }), muted)}>The {a?.type} editor mounts here (form designer, Workflow Studio, …). {pkg.author === me ? '' : 'Read-only: you are not the author.'}</div>
        )}
        <h3 className={css({ fontWeight: 'semibold' })}>Checks</h3>
        <Checks pkg={pkg} />
      </section>
      <aside className={cx(box, css({ p: '4', display: 'flex', flexDirection: 'column', gap: '3' }))}>
        <ol className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          {STAGES.filter((s) => s.key !== 'changes-requested').map((s) => {
            const order = ['draft', 'in-review', 'approved', 'published', 'active']
            const at = order.indexOf(pkg.stage === 'changes-requested' ? 'draft' : pkg.stage)
            const i = order.indexOf(s.key)
            return (
              <li key={s.key} className={css({ fontWeight: i === at ? 'bold' : 'normal', color: i > at ? 'fg.muted' : 'fg.default' })}>
                {i < at ? '✓' : i === at ? '●' : '○'} {s.label}
              </li>
            )
          })}
        </ol>
        <Actions pkg={pkg} me={me} vertical />
        <Approvals pkg={pkg} />
        <h3 className={css({ fontWeight: 'semibold' })}>Comments</h3>
        <History pkg={pkg} />
        <textarea placeholder="Comment…" className={css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '2', minHeight: '5rem' })} />
      </aside>
    </div>
  )
}

// C: a plain table of packages, each draft as one page read top to bottom: what changed, whether
// it is safe, who must act and what they said.
function VariantC({ me }: { me: string }) {
  const [pkg, setPkg] = useState<Pkg | undefined>()
  if (pkg === undefined) {
    return (
      <table className={css({ width: '100%', borderCollapse: 'collapse', '& td, & th': { borderBottom: '1px solid', borderColor: 'border', p: '2', textAlign: 'left' } })}>
        <thead>
          <tr>
            <th>Package</th>
            <th>Version</th>
            <th>Status</th>
            <th>Author</th>
            <th>Next step</th>
            <th>Active</th>
          </tr>
        </thead>
        <tbody>
          {PACKAGES.map((p) => {
            const next = actions(p, me).find((a) => a.allowed && a.label !== 'Run checks' && a.label !== 'Edit')
            return (
              <tr key={p.name}>
                <td>
                  <button type="button" onClick={() => setPkg(p)} className={css({ color: 'accent.text', textDecoration: 'underline', cursor: 'pointer' })}>
                    {p.title}
                  </button>
                  <div className={muted}>
                    {p.name} · {p.kind}
                  </div>
                </td>
                <td>
                  {p.version} <span className={muted}>({p.bump})</span>
                </td>
                <td>{stageLabel(p.stage)}</td>
                <td>{person(p.author)?.name}</td>
                <td>{next === undefined ? <span className={muted}>Nothing for you</span> : <strong>{next.label}</strong>}</td>
                <td>{p.activations.length === 0 ? <span className={muted}>—</span> : p.activations.map((x) => x.scope).join(', ')}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
    )
  }
  const failing = pkg.checks.filter((c) => !c.ok)
  return (
    <article className={css({ maxWidth: '3xl', display: 'flex', flexDirection: 'column', gap: '5', pb: '16' })}>
      <button type="button" className={ghost} onClick={() => setPkg(undefined)}>
        ← All packages
      </button>
      <header>
        <h2 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>
          {pkg.title} <span className={muted}>{pkg.version}</span>
        </h2>
        <p>
          <span className={chip}>{stageLabel(pkg.stage)}</span> {pkg.bump} change from {pkg.previous}, by {person(pkg.author)?.name}
        </p>
      </header>
      <section>
        <h3 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>1. What changed</h3>
        {pkg.artefacts
          .filter((a) => a.change !== 'unchanged')
          .map((a) => (
            <div key={a.type + a.name} className={cx(box, css({ p: '3', mt: '2' }))}>
              <div>
                <span className={chip}>{a.change}</span> {a.type} <strong>{a.name}</strong>
              </div>
              <p>{a.summary}</p>
            </div>
          ))}
      </section>
      <section>
        <h3 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>2. Is it safe? {failing.length === 0 ? <span className={css({ color: 'success.text' })}>All checks pass</span> : <span className={css({ color: 'danger.text' })}>{failing.length} failing</span>}</h3>
        <Checks pkg={pkg} />
      </section>
      <section>
        <h3 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>3. Who must act</h3>
        <Approvals pkg={pkg} />
        <Actions pkg={pkg} me={me} />
      </section>
      <section>
        <h3 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>4. Discussion</h3>
        <History pkg={pkg} />
        <textarea placeholder="Comment…" className={css({ width: '100%', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '2', minHeight: '5rem', mt: '2' })} />
      </section>
    </article>
  )
}

const VARIANTS = [
  { key: 'A', name: 'Lifecycle board', View: VariantA },
  { key: 'B', name: 'Three-pane workbench', View: VariantB },
  { key: 'C', name: 'List, then a draft as one page', View: VariantC },
] as const

export function WorkspacePrototype() {
  const [variant, setVariant] = useState(() => new URLSearchParams(window.location.search).get('variant') ?? 'A')
  const [me, setMe] = useState('okello')
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
      const t = e.target as HTMLElement
      if (t.closest('input, textarea, select, [contenteditable]')) return
      if (e.key === 'ArrowLeft') go(-1)
      if (e.key === 'ArrowRight') go(1)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
  const View = current.View
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <header className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '3' })}>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>Studio: packages</h1>
        <Me me={me} setMe={setMe} />
      </header>
      <View me={me} />
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
