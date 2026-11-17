// PROTOTYPE (#544), throwaway: the Report designer on /prototype/report-designer.
// Round 2: layout B (outline on the left, the Report always computed on the right, the DHIS2
// mapping as one sheet below). Layouts A (the form is the editor) and C (steps) are in this
// branch's history.
// Each section has its own Disaggregations, so indicators broken down the same way share a section;
// each Disaggregation is laid out as rows or as columns; a section can show column totals, row
// totals and % change from the previous period. A section can instead be a line list.
// Round 4: line lists are one row per enrollment (concept columns pick latest, first, all or a
// count of their visits' values) or per visit, with any attribute or any concept as a column.
// Round 3: the outline's sections are an accordion, one open at a time; clicking a section's title
// or a cell in the preview opens that section.
import { css, cx } from '@health/design-system/css'
import { useState } from 'react'

import { PERIOD_TYPES, type Indicator, type Period, type PeriodType } from './indicatorData'
import {
  CATALOGUE,
  COCS,
  DATA_ELEMENTS,
  DATA_SETS,
  DISAGGS,
  ATTRIBUTES,
  CONCEPTS,
  PICKS,
  PROGRAMS,
  attribute,
  concept,
  listRows,
  listValue,
  INITIAL_REPORT,
  LEVELS,
  PLACES,
  axisOf,
  canonical,
  cell,
  cellKey,
  change,
  cross,
  decD,
  disaggLabel,
  encD,
  facilityOf,
  facilityOnly,
  indicator,
  isCount,
  mappingFor,
  placeOf,
  previous,
  sentCells,
  validate,
  type Axis,
  type Issue,
  type Level,
  type ListBase,
  type ListColumn,
  type ListSection,
  type Pick,
  type Mapping,
  type Report,
  type Row,
  type Section,
  type TableSection,
  type Test,
} from './reportData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const small = cx(ghost, css({ minHeight: 'auto', py: '0', px: '2', fontSize: 'xs' }))
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const h3 = css({ fontSize: 'md', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs', color: 'fg.muted', wordBreak: 'break-word' })
const bad = css({ color: 'danger.text' })
const warn = css({ color: 'warning.text' })
const row = css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap', fontSize: 'sm' })
const th = css({ border: '1px solid', borderColor: 'border', px: '2', py: '1', fontWeight: 'semibold', fontSize: 'xs', bg: 'bg.subtle', textAlign: 'center' })
const tdL = css({ border: '1px solid', borderColor: 'border', px: '2', py: '1', fontSize: 'sm', textAlign: 'left' })
const td = css({ border: '1px solid', borderColor: 'border', px: '2', py: '1', fontSize: 'sm', textAlign: 'right', cursor: 'pointer', _hover: { bg: 'bg.subtle' } })
const totalCell = css({ fontWeight: 'semibold', bg: 'bg.subtle' })

let next = 10
const newId = (p: string) => `${p}${(next += 1)}`
const TODAY = '2026-11-18'
const LEVEL_LABEL: Record<Level, string> = { facility: 'Facility', district: 'District', region: 'Region', national: 'National' }

// --- State ----------------------------------------------------------------------------------------------

type Pos = { section: string; row: string; rk: string; ck: string; tests: Test[]; total: boolean } | null
type M = {
  r: Report
  set: (r: Report) => void
  period: Period
  setPeriod: (p: Period) => void
  place: string
  setPlace: (p: string) => void
  pos: Pos
  setPos: (p: Pos) => void
  /** The one section open in the outline (an accordion). */
  open: string | null
  setOpen: (id: string | null) => void
  issues: Issue[]
}
function useModel(): M {
  const [r, setR] = useState(INITIAL_REPORT)
  const [period, setPeriod] = useState<Period>(PERIOD_TYPES[1]?.periods[1] as Period)
  const [place, setPlace] = useState('kisenyi')
  const [pos, setPosOnly] = useState<Pos>(null)
  const [open, setOpen] = useState<string | null>(INITIAL_REPORT.sections[0]?.id ?? null)
  const setPos = (p: Pos) => {
    setPosOnly(p)
    if (p) setOpen(p.section)
  }
  const set = (x: Report) => {
    setR(x)
    if (x.periodType !== period.type) setPeriod(PERIOD_TYPES.find((p) => p.id === x.periodType)?.periods[0] as Period)
    if (!x.levels.includes(placeOf(place)?.level ?? 'facility')) setPlace(PLACES.find((p) => x.levels.includes(p.level))?.id ?? place)
  }
  return { r, set, period, setPeriod, place, setPlace, pos, setPos, open, setOpen, issues: validate(r) }
}

const move = <T,>(list: T[], i: number, by: number) => {
  const j = i + by
  if (j < 0 || j >= list.length) return list
  const out = [...list]
  const [x] = out.splice(i, 1)
  if (x !== undefined) out.splice(j, 0, x)
  return out
}
const putSection = (r: Report, s: Section): Report => ({ ...r, sections: r.sections.map((x) => (x.id === s.id ? s : x)) })

// --- Left: the outline ------------------------------------------------------------------------------------

function Basics({ m }: { m: M }) {
  const { r, set } = m
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <label className={row}>
        Title
        <input className={cx(input, css({ flex: '1', minWidth: '14rem' }))} value={r.title} onChange={(e) => set({ ...r, title: e.target.value })} />
      </label>
      <label className={row}>
        Period type
        <select className={input} value={r.periodType} onChange={(e) => set({ ...r, periodType: e.target.value as PeriodType })}>
          {PERIOD_TYPES.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
      </label>
      <div className={row}>
        Runs for
        {LEVELS.map((l) => (
          <label key={l}>
            <input type="checkbox" checked={r.levels.includes(l)} onChange={(e) => set({ ...r, levels: e.target.checked ? LEVELS.filter((x) => x === l || r.levels.includes(x)) : r.levels.filter((x) => x !== l) })} /> {LEVEL_LABEL[l]}
          </label>
        ))}
      </div>
      <label className={row}>
        <input type="checkbox" checked={r.dhis2 !== null} onChange={(e) => set({ ...r, dhis2: e.target.checked ? (DATA_SETS[0]?.id ?? '') : null })} /> Sent to DHIS2
        {r.dhis2 === null ? null : (
          <select className={cx(input, css({ maxWidth: '18rem' }))} value={r.dhis2} aria-label="DHIS2 data set" onChange={(e) => set({ ...r, dhis2: e.target.value })}>
            {DATA_SETS.map((d) => (
              <option key={d.id} value={d.id}>
                {d.label}
              </option>
            ))}
          </select>
        )}
      </label>
    </div>
  )
}

const summary = (s: Section) =>
  s.kind === 'list'
    ? `Line list · one row per ${s.base === 'enrollments' ? 'enrollment' : 'visit'} · ${s.columns.length} columns`
    : [
        `Table · ${s.rows.length} indicator${s.rows.length === 1 ? '' : 's'}`,
        ...s.disaggs.map((x) => `${disaggLabel(x.d).replace(/ \((age bands|coded)\)$/, '')} as ${x.axis}`),
        ...(s.send ? ['sent to DHIS2'] : []),
      ].join(' · ')

function SectionHead({ m, s, i }: { m: M; s: Section; i: number }) {
  const { r, set } = m
  const isOpen = m.open === s.id
  return (
    <div className={row}>
      <button type="button" className={css({ cursor: 'pointer', fontSize: 'md', width: '6', flexShrink: 0 })} aria-expanded={isOpen} aria-label={isOpen ? 'Collapse section' : 'Expand section'} onClick={() => m.setOpen(isOpen ? null : s.id)}>
        {isOpen ? '▾' : '▸'}
      </button>
      {isOpen ? (
        <input className={cx(input, css({ fontWeight: 'semibold', flex: '1', minWidth: '10rem' }))} value={s.title} aria-label="Section title" onChange={(e) => set(putSection(r, { ...s, title: e.target.value }))} />
      ) : (
        <button type="button" className={css({ flex: '1', textAlign: 'left', cursor: 'pointer', display: 'flex', flexDirection: 'column' })} onClick={() => m.setOpen(s.id)}>
          <span className={css({ fontWeight: 'semibold' })}>{s.title}</span>
          <span className={cx(muted, css({ fontSize: 'xs' }))}>{summary(s)}</span>
        </button>
      )}
      <button type="button" className={small} aria-label="Move up" onClick={() => set({ ...r, sections: move(r.sections, i, -1) })}>
        ↑
      </button>
      <button type="button" className={small} aria-label="Move down" onClick={() => set({ ...r, sections: move(r.sections, i, 1) })}>
        ↓
      </button>
      <button type="button" className={small} aria-label="Remove section" onClick={() => set({ ...r, sections: r.sections.filter((x) => x.id !== s.id) })}>
        ✕
      </button>
    </div>
  )
}

function TableEditor({ m, s }: { m: M; s: TableSection }) {
  const { r, set } = m
  const put = (x: TableSection) => set(putSection(r, x))
  const used = s.disaggs.map((x) => encD(x.d))
  const rowsD = axisOf(s, 'rows').length
  const colsD = axisOf(s, 'columns').length
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', pl: '3' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        <span className={muted}>Disaggregations (shared by every row in this section)</span>
        {s.disaggs.map((x, i) => (
          <div key={encD(x.d)} className={row}>
            <span className={css({ flex: '1' })}>{disaggLabel(x.d)}</span>
            <div role="radiogroup" aria-label={`Lay out ${disaggLabel(x.d)} as`} className={row}>
              {(['rows', 'columns'] as Axis[]).map((a) => (
                <label key={a}>
                  <input type="radio" checked={x.axis === a} onChange={() => put({ ...s, disaggs: s.disaggs.map((y, j) => (j === i ? { ...y, axis: a } : y)) })} /> {a}
                </label>
              ))}
            </div>
            <button type="button" className={small} aria-label="Move up" onClick={() => put({ ...s, disaggs: move(s.disaggs, i, -1) })}>
              ↑
            </button>
            <button type="button" className={small} aria-label="Remove" onClick={() => put({ ...s, disaggs: s.disaggs.filter((_, j) => j !== i) })}>
              ✕
            </button>
          </div>
        ))}
        <select className={cx(input, css({ alignSelf: 'start' }))} value="" aria-label="Add a disaggregation" onChange={(e) => e.target.value && put({ ...s, disaggs: [...s.disaggs, { d: decD(e.target.value), axis: 'columns' }] })}>
          <option value="">+ disaggregation</option>
          {DISAGGS.filter((d) => !used.includes(encD(d))).map((d) => (
            <option key={encD(d)} value={encD(d)}>
              {disaggLabel(d)}
            </option>
          ))}
        </select>
      </div>
      <div className={row}>
        <label title={rowsD === 0 ? 'Needs a Disaggregation laid out as rows' : 'A Total row under each indicator, over its row bands'}>
          <input type="checkbox" disabled={rowsD === 0} checked={s.colTotals} onChange={(e) => put({ ...s, colTotals: e.target.checked })} /> Column totals
        </label>
        <label title={colsD === 0 ? 'Needs a Disaggregation laid out as columns' : 'A Total column at the right, over the column bands'}>
          <input type="checkbox" disabled={colsD === 0} checked={s.rowTotals} onChange={(e) => put({ ...s, rowTotals: e.target.checked })} /> Row totals
        </label>
        <label>
          <input type="checkbox" checked={s.change} onChange={(e) => put({ ...s, change: e.target.checked })} /> % change from previous period
        </label>
        {r.dhis2 ? (
          <label>
            <input type="checkbox" checked={s.send} onChange={(e) => put({ ...s, send: e.target.checked })} /> Send to DHIS2
          </label>
        ) : null}
      </div>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        <span className={muted}>Indicators</span>
        {s.rows.map((rw, ri) => {
          const ind = indicator(rw.ind)
          return (
            <div key={rw.id} className={css({ display: 'flex', flexDirection: 'column' })}>
              <div className={row}>
                <input className={cx(input, css({ flex: '1', minWidth: '10rem' }))} value={rw.label} aria-label="Row label" onChange={(e) => put({ ...s, rows: s.rows.map((x) => (x.id === rw.id ? { ...x, label: e.target.value } : x)) })} />
                <button type="button" className={small} aria-label="Move up" onClick={() => put({ ...s, rows: move(s.rows, ri, -1) })}>
                  ↑
                </button>
                <button type="button" className={small} aria-label="Move down" onClick={() => put({ ...s, rows: move(s.rows, ri, 1) })}>
                  ↓
                </button>
                <button type="button" className={small} aria-label="Remove row" onClick={() => put({ ...s, rows: s.rows.filter((x) => x.id !== rw.id) })}>
                  ✕
                </button>
              </div>
              {ind ? <RowBadges m={m} ind={ind} s={s} /> : null}
            </div>
          )
        })}
        <IndicatorPicker onPick={(i) => put({ ...s, rows: [...s.rows, { id: newId('r'), ind: i.id, label: i.title }] })} />
      </div>
    </div>
  )
}

function RowBadges({ m, ind, s }: { m: M; ind: Indicator; s: TableSection }) {
  const above = m.r.levels.some((l) => l !== 'facility')
  const missing = s.disaggs.filter((x) => !ind.disaggregations.some((y) => encD(y) === encD(x.d)))
  return (
    <div className={cx(row, css({ pl: '2', gap: '2' }))}>
      <span className={mono} title={canonical(ind)}>
        {ind.id}|{ind.version} · {ind.calc}
      </span>
      {facilityOnly(ind) && above ? <span className={cx(bad, css({ fontSize: 'xs' }))}>facility only</span> : null}
      {m.r.dhis2 && s.send && !isCount(ind) ? <span className={cx(warn, css({ fontSize: 'xs' }))}>not a count, not sent</span> : null}
      {missing.length > 0 ? <span className={cx(muted, css({ fontSize: 'xs' }))}>doesn't list {missing.map((x) => disaggLabel(x.d)).join(', ')}</span> : null}
    </div>
  )
}

function IndicatorPicker({ onPick, label = '+ indicator' }: { onPick: (i: Indicator) => void; label?: string }) {
  return (
    <select
      className={cx(input, css({ fontSize: 'xs', alignSelf: 'start' }))}
      value=""
      aria-label="Add an indicator"
      onChange={(e) => {
        const i = indicator(e.target.value)
        if (i) onPick(i)
      }}
    >
      <option value="">{label}</option>
      {CATALOGUE.map((i) => (
        <option key={i.id} value={i.id}>
          {i.title} · {i.calc} · {i.version}
        </option>
      ))}
    </select>
  )
}

function ListEditor({ m, s }: { m: M; s: ListSection }) {
  const put = (x: ListSection) => m.set(putSection(m.r, x))
  const [q, setQ] = useState('')
  const program = PROGRAMS.find((x) => x.id === s.program)
  const add = (src: ListColumn['src'], label: string) => put({ ...s, columns: [...s.columns, { id: newId('c'), src, label }] })
  const used = (kind: string, id: string) => s.columns.some((c) => c.src.kind === kind && c.src.id === id)
  const matches = q.trim() === '' ? [] : CONCEPTS.filter((c) => `${c.label} ${c.code}`.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 8)
  const groups = ['Person', 'Enrollment', 'Visit'] as const
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', pl: '3' })}>
      <div role="radiogroup" aria-label="One row per" className={row}>
        One row per
        {(['enrollments', 'visits'] as ListBase[]).map((b) => (
          <label key={b}>
            <input type="radio" checked={s.base === b} onChange={() => put({ ...s, base: b, columns: s.columns.filter((c) => c.src.kind === 'concept' || (attribute(c.src.id)?.base ?? b) === b) })} /> {b === 'enrollments' ? 'enrollment' : 'visit'}
          </label>
        ))}
      </div>
      <label className={row}>
        Program
        <select className={cx(input, css({ maxWidth: '18rem' }))} value={s.program} onChange={(e) => put({ ...s, program: e.target.value, visitTypes: [] })}>
          {PROGRAMS.filter((x) => s.base === 'visits' || x.id !== 'any').map((x) => (
            <option key={x.id} value={x.id}>
              {x.label}
            </option>
          ))}
        </select>
      </label>
      {s.base === 'visits' ? (
        <div className={row}>
          Visits in the period of type
          {(program?.visitTypes ?? []).map((t) => (
            <label key={t}>
              <input type="checkbox" checked={s.visitTypes.includes(t)} onChange={(e) => put({ ...s, visitTypes: e.target.checked ? [...s.visitTypes, t] : s.visitTypes.filter((x) => x !== t) })} /> {t}
            </label>
          ))}
          {s.visitTypes.length === 0 ? <span className={muted}>(none ticked: all)</span> : null}
        </div>
      ) : (
        <div role="radiogroup" aria-label="Which enrollments" className={row}>
          Enrollments
          <label>
            <input type="radio" checked={s.when === 'inPeriod'} onChange={() => put({ ...s, when: 'inPeriod' })} /> started in the period
          </label>
          <label>
            <input type="radio" checked={s.when === 'active'} onChange={() => put({ ...s, when: 'active' })} /> active during the period
          </label>
        </div>
      )}
      <label className={row}>
        Only those counted by
        <select className={cx(input, css({ maxWidth: '16rem' }))} value={s.ind ?? ''} onChange={(e) => put({ ...s, ind: e.target.value || null })}>
          <option value="">(no indicator: everyone above)</option>
          {CATALOGUE.map((i) => (
            <option key={i.id} value={i.id}>
              {i.title}
            </option>
          ))}
        </select>
      </label>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        <span className={muted}>Columns</span>
        {s.columns.map((c, i) => {
          const con = c.src.kind === 'concept' ? concept(c.src.id) : undefined
          const attr = c.src.kind === 'attr' ? attribute(c.src.id) : undefined
          return (
            <div key={c.id} className={css({ display: 'flex', flexDirection: 'column' })}>
              <div className={row}>
                <input className={cx(input, css({ flex: '1', minWidth: '8rem' }))} value={c.label} aria-label="Column heading" onChange={(e) => put({ ...s, columns: s.columns.map((x) => (x.id === c.id ? { ...x, label: e.target.value } : x)) })} />
                {c.src.kind === 'concept' && s.base === 'enrollments' ? (
                  <select className={cx(input, css({ fontSize: 'xs' }))} aria-label="Which value" value={c.src.pick} onChange={(e) => put({ ...s, columns: s.columns.map((x) => (x.id === c.id && x.src.kind === 'concept' ? { ...x, src: { ...x.src, pick: e.target.value as Pick } } : x)) })}>
                    {PICKS.map((k) => (
                      <option key={k.id} value={k.id}>
                        {k.label}
                      </option>
                    ))}
                  </select>
                ) : null}
                <button type="button" className={small} aria-label="Move up" onClick={() => put({ ...s, columns: move(s.columns, i, -1) })}>
                  ↑
                </button>
                <button type="button" className={small} aria-label="Remove column" onClick={() => put({ ...s, columns: s.columns.filter((x) => x.id !== c.id) })}>
                  ✕
                </button>
              </div>
              <span className={cx(mono, css({ pl: '2' }))}>{con ? `concept ${con.code}` : `${attr?.group.toLowerCase() ?? ''} attribute`}</span>
            </div>
          )
        })}
        <select className={cx(input, css({ fontSize: 'xs', alignSelf: 'start' }))} value="" aria-label="Add an attribute column" onChange={(e) => {
          const at = attribute(e.target.value)
          if (at) add({ kind: 'attr', id: at.id }, at.label)
        }}>
          <option value="">+ attribute</option>
          {groups.map((g) => (
            <optgroup key={g} label={g}>
              {ATTRIBUTES.filter((x) => x.group === g && (x.base ?? s.base) === s.base && !used('attr', x.id)).map((x) => (
                <option key={x.id} value={x.id}>
                  {x.label}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
        <input className={cx(input, css({ fontSize: 'xs' }))} placeholder="+ concept: search any concept (e.g. haemoglobin, blood pressure)" value={q} onChange={(e) => setQ(e.target.value)} />
        {matches.map((c) => (
          <button key={c.id} type="button" className={cx(small, css({ alignSelf: 'start' }))} disabled={used('concept', c.id)} onClick={() => {
            add({ kind: 'concept', id: c.id, pick: 'latest' }, c.unit ? `${c.label} (${c.unit})` : c.label)
            setQ('')
          }}>
            {c.label} <span className={mono}>{c.code}</span>
          </button>
        ))}
      </div>
      <span className={muted}>Patient-level, so facility level only; never sent to DHIS2.</span>
    </div>
  )
}

function Outline({ m }: { m: M }) {
  const { r, set } = m
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      {r.sections.map((s, i) => (
        <div key={s.id} className={css({ display: 'flex', flexDirection: 'column', gap: '2', borderTop: i === 0 ? 'none' : '1px solid', borderColor: 'border', pt: i === 0 ? '0' : '3' })}>
          <SectionHead m={m} s={s} i={i} />
          {m.open !== s.id ? null : s.kind === 'table' ? <TableEditor m={m} s={s} /> : <ListEditor m={m} s={s} />}
        </div>
      ))}
      <div className={row}>
        <button type="button" className={ghost} onClick={() => {
            const id = newId('s')
            set({ ...r, sections: [...r.sections, { kind: 'table', id, title: 'New section', disaggs: [], rows: [], colTotals: false, rowTotals: false, change: false, send: false }] })
            m.setOpen(id)
          }}>
          + table section
        </button>
        <button type="button" className={ghost} onClick={() => {
            const id = newId('s')
            set({ ...r, sections: [...r.sections, { kind: 'list', id, title: 'New line list', base: 'visits', program: 'anc', visitTypes: [], when: 'inPeriod', ind: null, columns: [{ id: newId('c'), src: { kind: 'attr', id: 'name' }, label: 'Name' }, { id: newId('c'), src: { kind: 'attr', id: 'start' }, label: 'Visit date' }] }] })
            m.setOpen(id)
          }}>
          + line list
        </button>
      </div>
    </div>
  )
}

// --- Right: the Report, computed ------------------------------------------------------------------------

function PreviewControls({ m }: { m: M }) {
  const periods = PERIOD_TYPES.find((p) => p.id === m.r.periodType)?.periods ?? []
  return (
    <div className={row}>
      Preview
      <select className={input} aria-label="Period" value={m.period.label} onChange={(e) => m.setPeriod(periods.find((p) => p.label === e.target.value) ?? m.period)}>
        {periods.map((p) => (
          <option key={p.label} value={p.label}>
            {p.label}
          </option>
        ))}
      </select>
      at
      <select className={input} aria-label="Place" value={m.place} onChange={(e) => m.setPlace(e.target.value)}>
        {PLACES.filter((p) => m.r.levels.includes(p.level)).map((p) => (
          <option key={p.id} value={p.id}>
            {p.label} ({p.level})
          </option>
        ))}
      </select>
      <span className={muted}>over test patients; the product reads stored Indicator Reports</span>
    </div>
  )
}

function ValueCell({ m, s, rw, ind, rk, ck, tests, total }: { m: M; s: TableSection; rw: Row; ind: Indicator; rk: string; ck: string; tests: Test[]; total: boolean }) {
  const { period, place, pos, setPos, r } = m
  const v = cell(ind, tests, period, place)
  const delta = s.change ? change(v, cell(ind, tests, previous(period), place)) : ''
  const selected = pos?.section === s.id && pos.row === rw.id && pos.rk === rk && pos.ck === ck
  const unmapped = Boolean(r.dhis2) && s.send && !total && isCount(ind) && ![rk, ck].some((k) => k.split(' × ').includes('not recorded')) && !mappingFor(r.mappings[cellKey(rw.id, rk, ck)], TODAY)
  return (
    <td
      className={cx(td, total ? totalCell : '', v.why ? muted : '', unmapped ? css({ boxShadow: 'inset 3px 0 0 token(colors.danger.text)' }) : '', selected ? css({ outline: '2px solid', outlineColor: 'accent.default', outlineOffset: '-2px' }) : '')}
      title={unmapped ? 'No DHIS2 mapping' : v.why}
      onClick={() => setPos(selected ? null : { section: s.id, row: rw.id, rk, ck, tests, total })}
    >
      {v.text}
      {delta ? <div className={css({ fontSize: '2xs', color: delta.startsWith('▼') ? 'danger.text' : 'fg.muted' })}>{delta}</div> : null}
    </td>
  )
}

function SectionTable({ m, s }: { m: M; s: TableSection }) {
  const { period } = m
  const rowDs = axisOf(s, 'rows')
  const colDs = axisOf(s, 'columns')
  const first = indicator(s.rows[0]?.ind ?? '')
  if (!first) return <p className={muted}>No indicators yet.</p>
  const colCombos = cross(colDs, first, period)
  const levels = Math.max(1, colDs.length)
  const rowTotal = s.rowTotals && colDs.length > 0
  const colTotal = s.colTotals && rowDs.length > 0
  const groups = (lvl: number) => {
    const out: { key: string; label: string; span: number }[] = []
    for (const c of colCombos) {
      const key = c.parts.slice(0, lvl + 1).join('|')
      const last = out.at(-1)
      if (last && last.key === key) last.span += 1
      else out.push({ key, label: c.parts[lvl] ?? '', span: 1 })
    }
    return out
  }
  return (
    <div className={css({ overflowX: 'auto' })}>
      <table className={css({ borderCollapse: 'collapse', width: '100%' })}>
        <thead>
          {Array.from({ length: levels }, (_, lvl) => (
            <tr key={lvl}>
              {lvl === 0 ? (
                <>
                  <th className={cx(th, css({ textAlign: 'left' }))} rowSpan={levels}>
                    Indicator
                  </th>
                  {rowDs.map((d) => (
                    <th key={encD(d)} className={th} rowSpan={levels}>
                      {disaggLabel(d).replace(/ \((age bands|coded)\)$/, '')}
                    </th>
                  ))}
                </>
              ) : null}
              {colDs.length === 0 ? (
                <th className={th}>Value</th>
              ) : (
                groups(lvl).map((g) => (
                  <th key={g.key} className={th} colSpan={g.span}>
                    {g.label}
                  </th>
                ))
              )}
              {lvl === 0 && rowTotal ? (
                <th className={th} rowSpan={levels}>
                  Total
                </th>
              ) : null}
            </tr>
          ))}
        </thead>
        <tbody>
          {s.rows.map((rw) => {
            const ind = indicator(rw.ind)
            if (!ind) return null
            const rowCombos = cross(rowDs, ind, period)
            const cols = cross(colDs, ind, period)
            const span = rowCombos.length + (colTotal ? 1 : 0)
            return [
              ...rowCombos.map((rc, k) => (
                <tr key={`${rw.id}${rc.key}`}>
                  {k === 0 ? (
                    <td className={cx(tdL, css({ verticalAlign: 'top' }))} rowSpan={span}>
                      {rw.label}
                    </td>
                  ) : null}
                  {rc.parts.map((p, j) => (
                    <td key={`${j}${p}`} className={tdL}>
                      {p}
                    </td>
                  ))}
                  {cols.map((cc) => (
                    <ValueCell key={cc.key} m={m} s={s} rw={rw} ind={ind} rk={rc.key} ck={cc.key} tests={[...rc.tests, ...cc.tests]} total={false} />
                  ))}
                  {rowTotal ? <ValueCell m={m} s={s} rw={rw} ind={ind} rk={rc.key} ck="Total" tests={rc.tests} total /> : null}
                </tr>
              )),
              colTotal ? (
                <tr key={`${rw.id}-total`}>
                  <td className={cx(tdL, totalCell)} colSpan={rowDs.length}>
                    Total
                  </td>
                  {cols.map((cc) => (
                    <ValueCell key={cc.key} m={m} s={s} rw={rw} ind={ind} rk="Total" ck={cc.key} tests={cc.tests} total />
                  ))}
                  {rowTotal ? <ValueCell m={m} s={s} rw={rw} ind={ind} rk="Total" ck="Total" tests={[]} total /> : null}
                </tr>
              ) : null,
            ]
          })}
        </tbody>
      </table>
    </div>
  )
}

function LineList({ m, s }: { m: M; s: ListSection }) {
  const place = placeOf(m.place)
  if (place && place.level !== 'facility') return <p className={muted}>Line lists show patient-level records and run at facility level only. Pick a facility to see this one.</p>
  const list = listRows(s, m.period, m.place)
  return (
    <div className={css({ overflowX: 'auto' })}>
      <table className={css({ borderCollapse: 'collapse', width: '100%' })}>
        <thead>
          <tr>
            {s.columns.map((c) => (
              <th key={c.id} className={cx(th, css({ textAlign: 'left' }))}>
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {list.length === 0 ? (
            <tr>
              <td className={cx(tdL, muted)} colSpan={s.columns.length}>
                No {s.base} in {m.period.label}.
              </td>
            </tr>
          ) : (
            list.map((r) => (
              <tr key={r.key}>
                {s.columns.map((c) => (
                  <td key={c.id} className={tdL}>
                    {listValue(c, r)}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
      </table>
      <span className={muted}>
        {list.length} {s.base} · {PROGRAMS.find((x) => x.id === s.program)?.label}
        {s.ind ? ` · counted by ${indicator(s.ind)?.title ?? s.ind}` : ''}
      </span>
    </div>
  )
}

function CellPanel({ m }: { m: M }) {
  const { pos, r, period, place } = m
  if (!pos) return <p className={muted}>Click a cell to see who it counted and its DHIS2 mapping.</p>
  const s = r.sections.find((x) => x.id === pos.section)
  const rw = s?.kind === 'table' ? s.rows.find((x) => x.id === pos.row) : undefined
  const ind = rw ? indicator(rw.ind) : undefined
  if (!s || s.kind !== 'table' || !rw || !ind) return null
  const v = cell(ind, pos.tests, period, place)
  const before = cell(ind, pos.tests, previous(period), place)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <h3 className={h3}>
        {rw.label} · {[pos.rk, pos.ck].filter(Boolean).join(' · ') || 'value'}
      </h3>
      <span className={mono}>{canonical(ind)}</span>
      <div className={row}>
        <strong>{v.text}</strong>
        {v.n !== null && v.d !== null ? (
          <span className={muted}>
            ({v.n} / {Number(v.d.toFixed(1))})
          </span>
        ) : null}
        <span className={muted}>
          {period.label} · {placeOf(place)?.label} · previous period {previous(period).label}: {before.text} {change(v, before)}
        </span>
      </div>
      {v.why ? <span className={muted}>{v.why}</span> : null}
      {v.patients.length > 0 ? (
        <ul className={cx(muted, css({ pl: '4', listStyle: 'disc' }))}>
          {v.patients.map((p) => (
            <li key={p.id}>
              {p.name} · {p.encounterType} · {p.dates.start} · {placeOf(facilityOf(p))?.label}
            </li>
          ))}
        </ul>
      ) : null}
      {r.dhis2 && s.send && !pos.total ? (
        <>
          <h3 className={h3}>DHIS2</h3>
          {isCount(ind) ? <MappingEditor m={m} k={cellKey(rw.id, pos.rk, pos.ck)} /> : <span className={warn}>This row is a {ind.calc}; DHIS2 data elements on this form hold counts.</span>}
        </>
      ) : null}
    </div>
  )
}

// --- Bottom: DHIS2 mapping sheet -------------------------------------------------------------------------

function mapText(list: Mapping[] | undefined, date: string) {
  const x = mappingFor(list, date)
  return x ? `${x.de} · ${COCS.find((c) => c.id === x.coc)?.label ?? x.coc}` : null
}

function MappingEditor({ m, k }: { m: M; k: string }) {
  const { r, set } = m
  const list = r.mappings[k] ?? []
  const put = (l: Mapping[]) => set({ ...r, mappings: { ...r.mappings, [k]: l } })
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      {list.map((x, i) => (
        <div key={`${x.de}${x.from}${i}`} className={row}>
          <select className={cx(input, css({ maxWidth: '16rem' }))} aria-label="Data element" value={x.de} onChange={(e) => put(list.map((y, j) => (j === i ? { ...y, de: e.target.value } : y)))}>
            {DATA_ELEMENTS.map((d) => (
              <option key={d.id} value={d.id}>
                {d.label}
              </option>
            ))}
          </select>
          <select className={input} aria-label="Category option combo" value={x.coc} onChange={(e) => put(list.map((y, j) => (j === i ? { ...y, coc: e.target.value } : y)))}>
            {COCS.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
          from
          <input className={input} type="date" value={x.from} onChange={(e) => put(list.map((y, j) => (j === i ? { ...y, from: e.target.value } : y)))} />
          to
          <input className={input} type="date" value={x.to} onChange={(e) => put(list.map((y, j) => (j === i ? { ...y, to: e.target.value } : y)))} />
          <button type="button" className={small} aria-label="Remove mapping" onClick={() => put(list.filter((_, j) => j !== i))}>
            ✕
          </button>
        </div>
      ))}
      <div>
        <button
          type="button"
          className={small}
          onClick={() => {
            const open = list.findIndex((x) => x.to === '')
            const closed = open === -1 ? list : list.map((x, j) => (j === open ? { ...x, to: '2026-06-30' } : x))
            put([...closed, { de: list.at(-1)?.de ?? 'AN01a', coc: list.at(-1)?.coc ?? 'default', from: open === -1 ? '2024-07-01' : '2026-07-01', to: '' }])
          }}
        >
          {list.length === 0 ? '+ mapping' : '+ new mapping from a date (form changed)'}
        </button>
      </div>
    </div>
  )
}

function MappingSheet({ m }: { m: M }) {
  const { r } = m
  if (!r.dhis2) return <p className={muted}>Not sent to DHIS2.</p>
  const sent = r.sections.filter((s): s is TableSection => s.kind === 'table' && s.send)
  if (sent.length === 0) return <p className={muted}>No section is sent. Tick “Send to DHIS2” on a section.</p>
  return (
    <table className={css({ borderCollapse: 'collapse', width: '100%', fontSize: 'sm' })}>
      <thead>
        <tr>
          {['Section', 'Indicator', 'Cell', 'In effect today', 'Mappings'].map((h) => (
            <th key={h} className={cx(th, css({ textAlign: 'left' }))}>
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {sent.flatMap((s) =>
          sentCells(s, m.period).map(({ rw, ind, rk, ck, needed }) => {
            const k = cellKey(rw.id, rk, ck)
            const t = mapText(r.mappings[k], TODAY)
            return (
              <tr key={`${s.id}${k}`}>
                <td className={tdL}>{s.title}</td>
                <td className={tdL}>{rw.label}</td>
                <td className={tdL}>{[rk, ck].filter(Boolean).join(' · ') || 'value'}</td>
                <td className={cx(tdL, t ? mono : needed ? bad : muted)}>{t ?? (!isCount(ind) ? `a ${ind.calc}, not sent` : needed ? 'unmapped' : 'not sent')}</td>
                <td className={tdL}>{isCount(ind) ? <MappingEditor m={m} k={k} /> : null}</td>
              </tr>
            )
          }),
        )}
      </tbody>
    </table>
  )
}

function Issues({ m }: { m: M }) {
  if (m.issues.length === 0) return <p className={muted}>No issues.</p>
  return (
    <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
      {m.issues.map((i, k) => (
        <li key={`${i.where}${k}`} className={i.level === 'error' ? bad : i.level === 'warning' ? warn : muted}>
          <strong>{i.where}:</strong> {i.text}
        </li>
      ))}
    </ul>
  )
}

// --- Page -----------------------------------------------------------------------------------------------

export function ReportDesignerPrototype() {
  const m = useModel()
  const errors = m.issues.filter((i) => i.level === 'error').length
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', pb: '16' })}>
      <header>
        <p className={muted}>
          ← {m.r.pkg} {m.r.version} · Reports
        </p>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{m.r.title}</h1>
        <p className={muted}>
          {PERIOD_TYPES.find((p) => p.id === m.r.periodType)?.label} · runs for {m.r.levels.map((l) => LEVEL_LABEL[l].toLowerCase()).join(', ') || 'no level'} · {m.r.dhis2 ? `sent to DHIS2 (${m.r.dhis2})` : 'not sent to DHIS2'}
        </p>
      </header>
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '30rem 1fr' }, gap: '4', alignItems: 'start' })}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <section className={cx(box, css({ p: '3' }))}>
            <Basics m={m} />
          </section>
          <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h2 className={h2}>Sections</h2>
            <Outline m={m} />
          </section>
          <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h2 className={h2}>Validation ({errors} errors)</h2>
            <Issues m={m} />
          </section>
        </div>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '4', position: { lg: 'sticky' }, top: '4' }))}>
          <PreviewControls m={m} />
          <h2 className={h2}>{m.r.title}</h2>
          {m.r.sections.map((s) => (
            <div key={s.id} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              <button type="button" className={cx(h3, css({ textAlign: 'left', cursor: 'pointer', color: m.open === s.id ? 'accent.text' : 'fg.default' }))} title="Open this section in the outline" onClick={() => m.setOpen(s.id)}>
                {s.title}
              </button>
              {s.kind === 'table' ? <SectionTable m={m} s={s} /> : <LineList m={m} s={s} />}
            </div>
          ))}
          <div className={row}>
            <button type="button" className={ghost}>
              Print / PDF
            </button>
            <button type="button" className={ghost}>
              CSV
            </button>
          </div>
          <div className={cx(box, css({ p: '3' }))}>
            <CellPanel m={m} />
          </div>
        </section>
      </div>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <h2 className={h2}>DHIS2 mapping</h2>
        <MappingSheet m={m} />
      </section>
    </section>
  )
}
