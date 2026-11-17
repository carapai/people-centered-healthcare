// PROTOTYPE (#563), throwaway: the health worker workspace on /prototype/workspace, three layouts
// switched with ?variant=A|B|C and the floating bar (← / → keys). Made-up OPD data; the role
// switcher in the status bar changes the modules and the default density.
// A: three panes. A module rail, the list (queue or inbox), and the chart, side by side: the
//    queue stays in sight while charting.
// B: home, then focus. A home page of tiles (queue, inbox, unsigned notes); opening a patient
//    gives the chart the whole screen, with "back to queue" and "next patient".
// C: patient tabs. Module tabs on top, open patients as tabs, the queue in a drawer, the inbox
//    in a slide-over, and critical results as a banner on every screen.
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import { CHARTS, INBOX, QUEUE, ROLES, priorityLabel, type Density, type InboxItem, type Priority, type QueueItem, type Role } from './workspaceData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const primary = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', bg: 'accent.default', color: 'accent.fg' })
const small = cx(ghost, css({ minHeight: 'auto', py: '0.5', px: '2', fontSize: 'xs' }))
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const row = css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap' })
const chip = css({ fontSize: 'xs', px: '1.5', py: '0', borderRadius: 'sm', border: '1px solid', borderColor: 'border', whiteSpace: 'nowrap' })
const PRIORITY_TONE: Record<Priority, string> = {
  emergency: css({ bg: 'danger.default', color: 'danger.fg', borderColor: 'danger.default' }),
  urgent: css({ color: 'warning.text', borderColor: 'warning.text' }),
  routine: css({ color: 'fg.muted' }),
}
const rank: Record<Priority, number> = { emergency: 0, urgent: 1, routine: 2 }

// --- State ----------------------------------------------------------------------------------------------

type M = {
  role: Role
  setRole: (r: Role) => void
  density: Density
  setDensity: (d: Density) => void
  central: boolean
  setCentral: (b: boolean) => void
  pending: number
  queue: QueueItem[]
  take: (id: string) => void
  finish: (id: string) => void
  open: string | null
  setOpen: (id: string | null) => void
  inbox: InboxItem[]
  ack: (id: string) => void
}
function useModel(): M {
  const [role, setRoleOnly] = useState<Role>('clinician')
  const [density, setDensity] = useState<Density>('compact')
  const [central, setCentral] = useState(true)
  const [pending, setPending] = useState(3)
  const [queue, setQueue] = useState(QUEUE)
  const [open, setOpen] = useState<string | null>(null)
  const [inbox, setInbox] = useState(INBOX)
  const setRole = (r: Role) => {
    setRoleOnly(r)
    setDensity(ROLES.find((x) => x.id === r)?.density ?? 'comfortable')
  }
  // Emergency first, then urgent, then longest waiting (no Array.sort in lint; insertion is enough here).
  const ordered: QueueItem[] = []
  for (const q of queue) {
    const at = ordered.findIndex((o) => rank[o.priority] > rank[q.priority] || (o.priority === q.priority && o.waited < q.waited))
    ordered.splice(at === -1 ? ordered.length : at, 0, q)
  }
  return {
    role,
    setRole,
    density,
    setDensity,
    central,
    setCentral,
    pending: central ? 0 : pending,
    queue: ordered,
    take: (id) => {
      setQueue(queue.map((q) => (q.id === id ? { ...q, status: 'mine' } : q)))
      setPending(pending + 1)
    },
    finish: (id) => {
      setQueue(queue.filter((q) => q.id !== id))
      setPending(pending + 2)
      setOpen(null)
    },
    open,
    setOpen,
    inbox,
    ack: (id) => setInbox(inbox.filter((i) => i.id !== id)),
  }
}
const pad = (m: M) => (m.density === 'compact' ? css({ py: '1', px: '2', fontSize: 'sm' }) : css({ py: '2.5', px: '3', fontSize: 'md' }))
const roleOf = (m: M) => ROLES.find((r) => r.id === m.role) ?? ROLES[0]

// --- Status bar (persistent, every screen) ------------------------------------------------------------------

function StatusBar({ m }: { m: M }) {
  return (
    <div role="status" className={cx(row, css({ justifyContent: 'space-between', bg: m.central ? 'bg.subtle' : 'warning.subtle', px: '3', py: '1.5', fontSize: 'sm', borderBottom: '1px solid', borderColor: 'border' }))}>
      <div className={row}>
        <strong>Kisenyi HC IV · OPD</strong>
        <span>Facility Edge (LAN)</span>
        <button type="button" className={small} title="Prototype: simulate the internet link" onClick={() => m.setCentral(!m.central)}>
          {m.central ? '● Central reachable' : '○ Central unreachable: working on this facility only'}
        </button>
        <span className={m.pending > 0 ? css({ color: 'warning.text' }) : muted}>{m.pending > 0 ? `${m.pending} changes waiting to sync` : 'All synced 10:42'}</span>
      </div>
      <div className={row}>
        <select className={cx(ghost, css({ minHeight: 'auto', py: '0.5', fontSize: 'sm' }))} aria-label="Signed in as" value={m.role} onChange={(e) => m.setRole(e.target.value as Role)}>
          {ROLES.map((r) => (
            <option key={r.id} value={r.id}>
              {r.label}
            </option>
          ))}
        </select>
        <select className={cx(ghost, css({ minHeight: 'auto', py: '0.5', fontSize: 'sm' }))} aria-label="Density" value={m.density} onChange={(e) => m.setDensity(e.target.value as Density)}>
          <option value="compact">Compact</option>
          <option value="comfortable">Comfortable</option>
        </select>
        <button type="button" className={small} title="Locks the screen; a PIN switches user">
          Lock
        </button>
      </div>
    </div>
  )
}

// --- Queue, inbox, chart ------------------------------------------------------------------------------

const hi = (b: boolean) => (b ? css({ color: 'danger.text', fontWeight: 'semibold' }) : '')
function Vitals({ q }: { q: QueueItem }) {
  if (!q.vitals) return <span className={muted}>no triage yet</span>
  const v = q.vitals
  return (
    <span className={css({ fontFamily: 'mono', fontSize: 'xs' })}>
      <span className={hi(v.temp >= 38.5)}>T {v.temp}</span> · BP <span className={hi(v.bp !== '-' && Number(v.bp.split('/')[0]) >= 160)}>{v.bp}</span> · P {v.pulse} · <span className={hi(v.spo2 < 94)}>SpO₂ {v.spo2}</span> · {v.weight} kg
    </span>
  )
}

function Payment({ q }: { q: QueueItem }) {
  if (q.payment === 'free') return <span className={chip}>Free care</span>
  if (q.payment === 'cleared') return <span className={chip}>Paid</span>
  if (q.priority === 'emergency') return <span className={cx(chip, css({ color: 'warning.text' }))}>Payment deferred (emergency)</span>
  return <span className={cx(chip, css({ color: 'warning.text' }))}>Awaiting payment</span>
}

function QueueList({ m, onOpen, limit }: { m: M; onOpen: (id: string) => void; limit?: number }) {
  const list = limit ? m.queue.slice(0, limit) : m.queue
  return (
    <ol className={css({ display: 'flex', flexDirection: 'column' })} aria-label="Queue">
      {list.map((q) => (
        <li key={q.id} className={cx(pad(m), css({ borderBottom: '1px solid', borderColor: 'border', display: 'flex', flexDirection: 'column', gap: '0.5', cursor: 'pointer', bg: m.open === q.id ? 'bg.subtle' : undefined, _hover: { bg: 'bg.subtle' } }))} onClick={() => onOpen(q.id)}>
          <div className={cx(row, css({ justifyContent: 'space-between' }))}>
            <span className={row}>
              <span className={cx(chip, PRIORITY_TONE[q.priority])}>{priorityLabel[q.priority]}</span>
              <strong>{q.name}</strong>
              <span className={muted}>
                {q.age} {q.sex}
              </span>
            </span>
            <span className={q.waited >= 45 ? css({ color: 'danger.text', fontSize: 'sm' }) : muted}>{q.waited} min</span>
          </div>
          <span>{q.reason}</span>
          {m.role !== 'receptionist' ? <Vitals q={q} /> : null}
          <div className={row}>
            {q.flags.map((f) => (
              <span key={f} className={cx(chip, f.startsWith('Allergy') ? css({ color: 'danger.text', borderColor: 'danger.text' }) : '')}>
                {f}
              </span>
            ))}
            <Payment q={q} />
            <span className={css({ ml: 'auto', fontSize: 'xs' })}>
              {q.status === 'mine' ? (
                <strong>Yours</strong>
              ) : q.status === 'taken' ? (
                <span className={muted}>With {q.takenBy}</span>
              ) : (
                <button
                  type="button"
                  className={small}
                  onClick={(e) => {
                    e.stopPropagation()
                    m.take(q.id)
                    onOpen(q.id)
                  }}
                >
                  Take
                </button>
              )}
            </span>
          </div>
        </li>
      ))}
      {list.length === 0 ? <li className={cx(pad(m), muted)}>No one waiting.</li> : null}
    </ol>
  )
}

const KIND: Record<InboxItem['kind'], string> = { result: 'Result', referral: 'Referral', approval: 'Approval', request: 'Patient request', documentation: 'Documentation' }
function InboxList({ m, limit, onOpen }: { m: M; limit?: number; onOpen: (patient: string) => void }) {
  const ordered = [...m.inbox.filter((i) => i.critical), ...m.inbox.filter((i) => !i.critical)]
  const list = limit ? ordered.slice(0, limit) : ordered
  return (
    <ol className={css({ display: 'flex', flexDirection: 'column' })} aria-label="Inbox">
      {list.map((i) => (
        <li key={i.id} className={cx(pad(m), css({ borderBottom: '1px solid', borderColor: 'border', display: 'flex', flexDirection: 'column', gap: '0.5', bg: i.critical ? 'danger.subtle' : undefined }))}>
          <div className={cx(row, css({ justifyContent: 'space-between' }))}>
            <span className={row}>
              <span className={cx(chip, i.critical ? PRIORITY_TONE.emergency : '')}>{i.critical ? 'Critical' : KIND[i.kind]}</span>
              <strong>{i.title}</strong>
            </span>
            <span className={muted}>{i.since}</span>
          </div>
          <div className={row}>
            <button type="button" className={css({ textDecoration: 'underline', cursor: 'pointer', fontSize: 'sm' })} onClick={() => onOpen(i.patient)}>
              {i.patient}
            </button>
            <button type="button" className={cx(small, css({ ml: 'auto' }))} onClick={() => m.ack(i.id)}>
              {i.kind === 'approval' ? 'Approve' : i.kind === 'documentation' ? 'Sign' : 'Mark reviewed'}
            </button>
          </div>
        </li>
      ))}
      {list.length === 0 ? <li className={cx(pad(m), muted)}>Nothing in the inbox.</li> : null}
    </ol>
  )
}

const TABS = ['Summary', 'Note', 'Orders', 'Results'] as const
function ChartView({ m, id, compactHeader = false }: { m: M; id: string; compactHeader?: boolean }) {
  const q = QUEUE.find((x) => x.id === id)
  const c = CHARTS[id]
  const [tab, setTab] = useState<(typeof TABS)[number]>('Summary')
  if (!q || !c) return <p className={muted}>Pick a patient.</p>
  const allergies = c.allergies.length === 0 ? 'No known allergies' : c.allergies.join(', ')
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <div className={cx(box, css({ p: compactHeader ? '2' : '3', display: 'flex', flexDirection: 'column', gap: '1' }))}>
        <div className={cx(row, css({ justifyContent: 'space-between' }))}>
          <span className={row}>
            <span className={cx(chip, PRIORITY_TONE[q.priority])}>{priorityLabel[q.priority]}</span>
            <strong className={css({ fontSize: 'lg' })}>{q.name}</strong>
            <span className={muted}>
              {q.age} {q.sex} · UG-PID-{id.toUpperCase()}48
            </span>
          </span>
          <Payment q={q} />
        </div>
        <span className={c.allergies.length ? css({ color: 'danger.text', fontWeight: 'semibold', fontSize: 'sm' }) : muted}>{allergies}</span>
        <Vitals q={q} />
        <span className={css({ fontSize: 'sm' })}>Reason: {q.reason}</span>
      </div>
      <div role="tablist" className={row}>
        {TABS.map((t) => (
          <button key={t} role="tab" type="button" aria-selected={tab === t} className={cx(ghost, tab === t ? css({ bg: 'bg.subtle', fontWeight: 'semibold' }) : '')} onClick={() => setTab(t)}>
            {t}
            {t === 'Results' && c.results.some((r) => r.flag === 'critical') ? <span className={css({ color: 'danger.text' })}> !</span> : null}
          </button>
        ))}
      </div>
      <div className={cx(box, css({ p: '3', fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        {tab === 'Summary' ? (
          <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', md: '1fr 1fr' }, gap: '3' })}>
            {(
              [
                ['Conditions', c.conditions],
                ['Medications', c.medications],
                ['Programs', c.programs],
                ['Recent visits', c.encounters.map((e) => `${e.date}: ${e.what}`)],
              ] as [string, string[]][]
            ).map(([title, items]) => (
              <div key={title}>
                <h3 className={css({ fontWeight: 'semibold' })}>{title}</h3>
                {items.length === 0 ? <span className={muted}>None recorded</span> : (
                  <ul className={css({ pl: '4', listStyle: 'disc' })}>
                    {items.map((x) => (
                      <li key={x}>{x}</li>
                    ))}
                  </ul>
                )}
              </div>
            ))}
          </div>
        ) : null}
        {tab === 'Note' ? (
          <>
            {['Subjective', 'Objective', 'Assessment', 'Plan'].map((s) => (
              <label key={s} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
                {s}
                <textarea className={cx(box, css({ p: '2', minHeight: m.density === 'compact' ? '12' : '20' }))} defaultValue={s === 'Objective' && q.vitals ? `T ${q.vitals.temp}, BP ${q.vitals.bp}, P ${q.vitals.pulse}, SpO₂ ${q.vitals.spo2}` : ''} />
              </label>
            ))}
            <span className={muted}>Saved on this facility's edge as you type; signing makes it immutable (changes become amendments).</span>
          </>
        ) : null}
        {tab === 'Orders' ? (
          <div className={row}>
            <button type="button" className={ghost}>
              + Lab test
            </button>
            <button type="button" className={ghost}>
              + Prescription
            </button>
            <button type="button" className={ghost}>
              + Imaging
            </button>
            <button type="button" className={ghost}>
              + Referral
            </button>
            {c.allergies.length ? <span className={css({ color: 'danger.text' })}>Prescribing checks against: {allergies}</span> : null}
          </div>
        ) : null}
        {tab === 'Results' ? (
          c.results.length === 0 ? (
            <span className={muted}>No results.</span>
          ) : (
            <ul>
              {c.results.map((r) => (
                <li key={r.name} className={r.flag === 'critical' ? css({ color: 'danger.text', fontWeight: 'semibold' }) : r.flag ? css({ color: 'warning.text' }) : ''}>
                  {r.name}: {r.value} <span className={muted}>({r.when})</span>
                </li>
              ))}
            </ul>
          )
        ) : null}
      </div>
      <div className={row}>
        <button type="button" className={primary} onClick={() => m.finish(id)}>
          Done: send to pharmacy
        </button>
        <button type="button" className={ghost} onClick={() => m.finish(id)}>
          Done: send to lab and come back
        </button>
        <button type="button" className={ghost} onClick={() => m.finish(id)}>
          Done: discharge
        </button>
        <button type="button" className={ghost}>
          Hand over to…
        </button>
      </div>
    </div>
  )
}

const patientByName = (name: string) => QUEUE.find((q) => q.name === name)?.id ?? null

// --- Variant A: three panes ---------------------------------------------------------------------------

function ThreePanes({ m }: { m: M }) {
  const [mod, setMod] = useState('queue')
  const role = roleOf(m)
  const counts: Record<string, number> = { queue: m.queue.length, inbox: m.inbox.length, notes: m.inbox.filter((i) => i.kind === 'documentation').length }
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: '11rem 24rem 1fr', gap: '0', minHeight: '70vh', border: '1px solid', borderColor: 'border', borderRadius: 'md', overflow: 'hidden' })}>
      <nav aria-label="Modules" className={css({ bg: 'bg.subtle', display: 'flex', flexDirection: 'column', p: '2', gap: '1' })}>
        {role?.modules.map((x) => (
          <button key={x.id} type="button" className={cx(pad(m), css({ textAlign: 'left', borderRadius: 'md', cursor: 'pointer', display: 'flex', justifyContent: 'space-between', bg: mod === x.id ? 'bg.surface' : undefined, fontWeight: mod === x.id ? 'semibold' : 'normal' }))} onClick={() => setMod(x.id)}>
            {x.label}
            {counts[x.id] ? <span className={cx(chip, x.id === 'inbox' && m.inbox.some((i) => i.critical) ? PRIORITY_TONE.emergency : '')}>{counts[x.id]}</span> : null}
          </button>
        ))}
      </nav>
      <section className={css({ borderRight: '1px solid', borderColor: 'border', overflowY: 'auto' })}>
        <h2 className={cx(h2, css({ p: '2' }))}>{role?.modules.find((x) => x.id === mod)?.label}</h2>
        {mod === 'inbox' || mod === 'notes' ? <InboxList m={m} onOpen={(n) => m.setOpen(patientByName(n))} /> : <QueueList m={m} onOpen={m.setOpen} />}
      </section>
      <section className={css({ p: '3', overflowY: 'auto' })}>{m.open ? <ChartView m={m} id={m.open} /> : <p className={muted}>Pick a patient from the list. The list stays here while you chart.</p>}</section>
    </div>
  )
}

// --- Variant B: home, then focus -------------------------------------------------------------------------

function HomeFocus({ m }: { m: M }) {
  const role = roleOf(m)
  if (m.open) {
    const i = m.queue.findIndex((q) => q.id === m.open)
    const nextUp = m.queue.find((q, j) => j > i && q.status !== 'taken')
    return (
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <div className={cx(row, css({ justifyContent: 'space-between' }))}>
          <button type="button" className={ghost} onClick={() => m.setOpen(null)}>
            ← Home ({m.queue.length} waiting, {m.inbox.length} in inbox)
          </button>
          {m.inbox.some((x) => x.critical) ? <span className={cx(chip, PRIORITY_TONE.emergency)}>Critical result waiting</span> : null}
          {nextUp ? (
            <button type="button" className={ghost} onClick={() => m.setOpen(nextUp.id)}>
              Next: {nextUp.name} →
            </button>
          ) : null}
        </div>
        <ChartView m={m} id={m.open} />
      </div>
    )
  }
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '3fr 2fr' }, gap: '4' })}>
      <section className={box}>
        <h2 className={cx(h2, css({ p: '3', pb: '1' }))}>
          {role?.modules[0]?.label} · {m.queue.length}
        </h2>
        <QueueList m={m} onOpen={m.setOpen} />
      </section>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        {m.role !== 'receptionist' ? (
          <section className={box}>
            <h2 className={cx(h2, css({ p: '3', pb: '1' }))}>Inbox · {m.inbox.length}</h2>
            <InboxList m={m} limit={4} onOpen={(n) => m.setOpen(patientByName(n))} />
          </section>
        ) : null}
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '1' }))}>
          <h2 className={h2}>Today</h2>
          <span>Seen: 14 · Waiting: {m.queue.length} · Longest wait: {Math.max(0, ...m.queue.map((q) => q.waited))} min</span>
          <span className={muted}>Unsigned notes: {m.inbox.filter((i) => i.kind === 'documentation').length}</span>
        </section>
      </div>
    </div>
  )
}

// --- Variant C: patient tabs --------------------------------------------------------------------------

function PatientTabs({ m }: { m: M }) {
  const role = roleOf(m)
  const [tabs, setTabs] = useState<string[]>([])
  const [drawer, setDrawer] = useState(true)
  const [slide, setSlide] = useState(false)
  const openTab = (id: string | null) => {
    if (!id) return
    if (!tabs.includes(id)) setTabs([...tabs, id])
    m.setOpen(id)
  }
  const critical = m.inbox.find((i) => i.critical)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', position: 'relative' })}>
      {critical ? (
        <div role="alert" className={cx(row, css({ bg: 'danger.subtle', color: 'danger.text', border: '2px solid', borderColor: 'danger.text', fontWeight: 'semibold', px: '3', py: '1.5', borderRadius: 'md', justifyContent: 'space-between' }))}>
          <span>
            Critical: {critical.title} · {critical.patient}
          </span>
          <span className={row}>
            <button type="button" className={css({ textDecoration: 'underline', cursor: 'pointer' })} onClick={() => openTab(patientByName(critical.patient))}>
              Open
            </button>
            <button type="button" className={css({ textDecoration: 'underline', cursor: 'pointer' })} onClick={() => m.ack(critical.id)}>
              Mark reviewed
            </button>
          </span>
        </div>
      ) : null}
      <div className={cx(row, css({ borderBottom: '1px solid', borderColor: 'border', gap: '0' }))}>
        <button type="button" className={cx(ghost, css({ borderRadius: '0' }))} onClick={() => setDrawer(!drawer)}>
          {drawer ? '◂' : '▸'} {role?.modules[0]?.label} ({m.queue.length})
        </button>
        {tabs.map((t) => {
          const q = QUEUE.find((x) => x.id === t)
          return (
            <span key={t} className={cx(row, css({ px: '3', py: '1.5', borderBottom: '2px solid', borderColor: m.open === t ? 'accent.default' : 'transparent', gap: '1' }))}>
              <button type="button" className={css({ cursor: 'pointer', fontWeight: m.open === t ? 'semibold' : 'normal' })} onClick={() => m.setOpen(t)}>
                {q?.name}
              </button>
              <button
                type="button"
                aria-label="Close tab"
                className={css({ cursor: 'pointer', color: 'fg.muted' })}
                onClick={() => {
                  setTabs(tabs.filter((x) => x !== t))
                  if (m.open === t) m.setOpen(null)
                }}
              >
                ×
              </button>
            </span>
          )
        })}
        <button type="button" className={cx(ghost, css({ ml: 'auto', borderRadius: '0' }))} onClick={() => setSlide(!slide)}>
          Inbox ({m.inbox.length})
        </button>
      </div>
      <div className={css({ display: 'grid', gridTemplateColumns: drawer ? '22rem 1fr' : '1fr', gap: '3' })}>
        {drawer ? (
          <section className={cx(box, css({ overflowY: 'auto', maxHeight: '75vh' }))}>
            <QueueList m={m} onOpen={openTab} />
          </section>
        ) : null}
        <section>{m.open ? <ChartView m={m} id={m.open} compactHeader /> : <p className={muted}>Open a patient from the queue; each opens in its own tab.</p>}</section>
      </div>
      {slide ? (
        <aside className={cx(box, css({ position: 'absolute', right: '0', top: '10', width: '26rem', maxHeight: '75vh', overflowY: 'auto', boxShadow: 'lg', zIndex: '5' }))}>
          <div className={cx(row, css({ justifyContent: 'space-between', p: '2' }))}>
            <h2 className={h2}>Inbox</h2>
            <button type="button" className={small} onClick={() => setSlide(false)}>
              Close
            </button>
          </div>
          <InboxList
            m={m}
            onOpen={(n) => {
              openTab(patientByName(n))
              setSlide(false)
            }}
          />
        </aside>
      ) : null}
    </div>
  )
}

// --- Shell ---------------------------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'three panes', View: ThreePanes },
  { key: 'B', name: 'home, then focus', View: HomeFocus },
  { key: 'C', name: 'patient tabs', View: PatientTabs },
]

export function WorkspacePrototype() {
  const m = useModel()
  const [variant, setVariant] = useState(() => new URLSearchParams(window.location.search).get('variant') ?? 'A')
  const index = Math.max(
    0,
    VARIANTS.findIndex((v) => v.key === variant),
  )
  const current = VARIANTS[index] ?? VARIANTS[0]
  const go = (step: number) => {
    const target = VARIANTS[(index + step + VARIANTS.length) % VARIANTS.length] ?? VARIANTS[0]
    const url = new URL(window.location.href)
    url.searchParams.set('variant', target?.key ?? 'A')
    window.history.replaceState(null, '', url)
    setVariant(target?.key ?? 'A')
    m.setOpen(null)
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
  if (!current) return null
  const View = current.View
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '3', pb: '16', mx: '-6', mt: '-6' })}>
      <StatusBar m={m} />
      <div className={css({ px: '4' })}>
        <View key={current.key} m={m} />
      </div>
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
