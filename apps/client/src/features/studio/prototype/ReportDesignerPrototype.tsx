// PROTOTYPE (#544), throwaway: the Report designer on /prototype/report-designer, three layouts
// switched with ?variant=A|B|C and the floating bar (← / → keys). What a Report is comes from
// ADR 0070; only the designer's look is in question.
// A: the form is the editor. The table is drawn as it will print; edit headings and labels in
//    place, click a cell for its DHIS2 mapping and (in Preview) who it counted.
// B: outline and live preview. Settings, columns and rows as an outline on the left; the table
//    always computed on the right; the DHIS2 mapping as one sheet, a line per cell.
// C: steps. Basics → Rows → Columns → DHIS2 → Preview, with the issues found so far beside them.
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import { AGE_SETS, CODED, PERIOD_TYPES, disaggLabel, type Disagg, type Indicator, type Period, type PeriodType } from './indicatorData'
import {
  CATALOGUE,
  COCS,
  DATA_ELEMENTS,
  DATA_SETS,
  INITIAL_REPORT,
  LEVELS,
  PLACES,
  canonical,
  cell,
  cellKey,
  columns,
  facilityOf,
  facilityOnly,
  headerColumns,
  indicator,
  isCount,
  mappingFor,
  validate,
  type Col,
  type Issue,
  type Level,
  type Mapping,
  type Report,
  type Row,
} from './reportData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const small = cx(ghost, css({ minHeight: 'auto', py: '0', px: '2', fontSize: 'xs' }))
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs', color: 'fg.muted', wordBreak: 'break-word' })
const bad = css({ color: 'danger.text' })
const warn = css({ color: 'warning.text' })
const row = css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap', fontSize: 'sm' })
const th = css({ border: '1px solid', borderColor: 'border', px: '2', py: '1', fontWeight: 'semibold', fontSize: 'xs', bg: 'bg.subtle', textAlign: 'center' })
const td = css({ border: '1px solid', borderColor: 'border', px: '2', py: '1', fontSize: 'sm', textAlign: 'right', cursor: 'pointer', _hover: { bg: 'bg.subtle' } })

let next = 10
const newId = (p: string) => `${p}${(next += 1)}`
const TODAY = '2026-11-17'
const LEVEL_LABEL: Record<Level, string> = { facility: 'Facility', district: 'District', region: 'Region', national: 'National' }

// --- State ----------------------------------------------------------------------------------------------

type Pos = { row: string; col: string } | null
type M = {
  r: Report
  set: (r: Report) => void
  period: Period
  setPeriod: (p: Period) => void
  place: string
  setPlace: (p: string) => void
  pos: Pos
  setPos: (p: Pos) => void
  issues: Issue[]
}
function useModel(): M {
  const [r, setR] = useState(INITIAL_REPORT)
  const [period, setPeriod] = useState<Period>(PERIOD_TYPES[1]?.periods[1] as Period)
  const [place, setPlace] = useState('kisenyi')
  const [pos, setPos] = useState<Pos>(null)
  const set = (x: Report) => {
    setR(x)
    if (x.periodType !== period.type) setPeriod(PERIOD_TYPES.find((p) => p.id === x.periodType)?.periods[0] as Period)
    if (!x.levels.includes(PLACES.find((p) => p.id === place)?.level ?? 'facility')) setPlace(PLACES.find((p) => x.levels.includes(p.level))?.id ?? place)
  }
  return { r, set, period, setPeriod, place, setPlace, pos, setPos, issues: validate(r) }
}

const mapRows = (r: Report, f: (row: Row) => Row | null): Report => ({ ...r, sections: r.sections.map((s) => ({ ...s, rows: s.rows.map(f).filter((x): x is Row => x !== null) })) })
const move = <T,>(list: T[], i: number, by: number) => {
  const j = i + by
  if (j < 0 || j >= list.length) return list
  const out = [...list]
  const [x] = out.splice(i, 1)
  if (x !== undefined) out.splice(j, 0, x)
  return out
}
const encD = (d: Disagg) => (d.kind === 'age' ? `age:${d.set}` : `coded:${d.input}`)
const decD = (s: string): Disagg => {
  const [k = '', v = ''] = s.split(':')
  return k === 'age' ? { kind: 'age', set: v } : { kind: 'coded', input: v }
}
const DISAGGS: Disagg[] = [...AGE_SETS.map((a): Disagg => ({ kind: 'age', set: a.id })), ...CODED.map((c): Disagg => ({ kind: 'coded', input: c.id }))]

// --- Shared pieces ----------------------------------------------------------------------------------------

function Basics({ m }: { m: M }) {
  const { r, set } = m
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <label className={row}>
        Title
        <input className={cx(input, css({ flex: '1', minWidth: '16rem' }))} value={r.title} onChange={(e) => set({ ...r, title: e.target.value })} />
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
        <span className={muted}>one per Report; a quarterly version is a second Report</span>
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
          <select className={input} value={r.dhis2} aria-label="DHIS2 data set" onChange={(e) => set({ ...r, dhis2: e.target.value })}>
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

function Columns({ m }: { m: M }) {
  const { r, set } = m
  const used = r.columns.map(encD)
  return (
    <div className={row}>
      Columns
      {r.columns.length === 0 ? <span className={muted}>none: one Total column</span> : null}
      {r.columns.map((d, i) => (
        <span key={encD(d)} className={cx(box, css({ px: '2', py: '0.5', display: 'inline-flex', gap: '1', alignItems: 'center' }))}>
          {i === 1 ? '× ' : ''}
          {disaggLabel(d)}
          <button type="button" className={small} aria-label="Remove" onClick={() => set({ ...r, columns: r.columns.filter((_, j) => j !== i) })}>
            ✕
          </button>
        </span>
      ))}
      {r.columns.length < 2 ? (
        <select className={input} value="" aria-label="Add a disaggregation" onChange={(e) => e.target.value && set({ ...r, columns: [...r.columns, decD(e.target.value)] })}>
          <option value="">{r.columns.length === 0 ? '+ disaggregation' : '+ cross with…'}</option>
          {DISAGGS.filter((d) => !used.includes(encD(d))).map((d) => (
            <option key={encD(d)} value={encD(d)}>
              {disaggLabel(d)}
            </option>
          ))}
        </select>
      ) : (
        <span className={muted}>at most two crossed</span>
      )}
      <label>
        <input type="checkbox" checked={r.total} onChange={(e) => set({ ...r, total: e.target.checked })} /> Total column
      </label>
    </div>
  )
}

function IndicatorPicker({ onPick, label = '+ row' }: { onPick: (i: Indicator) => void; label?: string }) {
  return (
    <select className={cx(input, css({ fontSize: 'xs' }))} value="" aria-label="Add an indicator row" onChange={(e) => {
      const i = indicator(e.target.value)
      if (i) onPick(i)
    }}>
      <option value="">{label}</option>
      {CATALOGUE.map((i) => (
        <option key={i.id} value={i.id}>
          {i.title} · {i.calc} · {i.version}
        </option>
      ))}
    </select>
  )
}

function RowBadges({ m, ind }: { m: M; ind: Indicator }) {
  const above = m.r.levels.some((l) => l !== 'facility')
  return (
    <>
      <span className={mono} title={canonical(ind)}>
        {ind.id}|{ind.version} · {ind.calc}
      </span>
      {facilityOnly(ind) && above ? <span className={cx(bad, css({ fontSize: 'xs' }))}>facility only</span> : null}
      {m.r.dhis2 && !isCount(ind) ? <span className={cx(warn, css({ fontSize: 'xs' }))}>not a count</span> : null}
    </>
  )
}

function Outline({ m }: { m: M }) {
  const { r, set } = m
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      {r.sections.map((s, si) => (
        <div key={s.id} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          <div className={row}>
            <input className={cx(input, css({ fontWeight: 'semibold', flex: '1' }))} value={s.title} aria-label="Section title" onChange={(e) => set({ ...r, sections: r.sections.map((x) => (x.id === s.id ? { ...x, title: e.target.value } : x)) })} />
            <button type="button" className={small} onClick={() => set({ ...r, sections: move(r.sections, si, -1) })}>
              ↑
            </button>
            <button type="button" className={small} onClick={() => set({ ...r, sections: move(r.sections, si, 1) })}>
              ↓
            </button>
            <button type="button" className={small} onClick={() => set({ ...r, sections: r.sections.filter((x) => x.id !== s.id) })}>
              ✕
            </button>
          </div>
          {s.rows.map((rw, ri) => {
            const ind = indicator(rw.ind)
            return (
              <div key={rw.id} className={cx(row, css({ pl: '4' }))}>
                <input className={cx(input, css({ flex: '1', minWidth: '12rem' }))} value={rw.label} aria-label="Row label" onChange={(e) => set(mapRows(r, (x) => (x.id === rw.id ? { ...x, label: e.target.value } : x)))} />
                {ind ? <RowBadges m={m} ind={ind} /> : null}
                <button type="button" className={small} onClick={() => set({ ...r, sections: r.sections.map((x) => (x.id === s.id ? { ...x, rows: move(x.rows, ri, -1) } : x)) })}>
                  ↑
                </button>
                <button type="button" className={small} onClick={() => set({ ...r, sections: r.sections.map((x) => (x.id === s.id ? { ...x, rows: move(x.rows, ri, 1) } : x)) })}>
                  ↓
                </button>
                <button type="button" className={small} onClick={() => set(mapRows(r, (x) => (x.id === rw.id ? null : x)))}>
                  ✕
                </button>
              </div>
            )
          })}
          <div className={css({ pl: '4' })}>
            <IndicatorPicker onPick={(i) => set({ ...r, sections: r.sections.map((x) => (x.id === s.id ? { ...x, rows: [...x.rows, { id: newId('r'), ind: i.id, label: i.title }] } : x)) })} />
          </div>
        </div>
      ))}
      <button type="button" className={ghost} onClick={() => set({ ...r, sections: [...r.sections, { id: newId('s'), title: 'New section', rows: [] }] })}>
        + section
      </button>
    </div>
  )
}

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

function mapText(list: Mapping[] | undefined, date: string) {
  const m = mappingFor(list, date)
  if (!m) return null
  return `${m.de} · ${COCS.find((c) => c.id === m.coc)?.label ?? m.coc}`
}

/** The Report as a table. `show` decides what a cell says: the computed value, or its DHIS2 mapping. */
function Table({ m, show, compact = false }: { m: M; show: 'values' | 'mapping'; compact?: boolean }) {
  const { r, period, place, pos, setPos } = m
  const cols = headerColumns(r, period)
  const crossed = r.columns.length === 2
  const groups: { label: string; span: number }[] = []
  if (crossed)
    for (const c of cols) {
      const head = c.total ? 'Total' : (c.parts[0] ?? '')
      const last = groups.at(-1)
      if (last && last.label === head && !c.total) last.span += 1
      else groups.push({ label: head, span: 1 })
    }
  return (
    <div className={css({ overflowX: 'auto' })}>
      <table className={css({ borderCollapse: 'collapse', width: '100%', fontSize: compact ? 'xs' : 'sm' })}>
        <thead>
          {crossed ? (
            <tr>
              <th className={th} rowSpan={2} />
              {groups.map((g) => (
                <th key={g.label} className={th} colSpan={g.span} rowSpan={g.label === 'Total' ? 2 : 1}>
                  {g.label}
                </th>
              ))}
            </tr>
          ) : null}
          <tr>
            {crossed ? null : <th className={th} />}
            {cols
              .filter((c) => !(crossed && c.total))
              .map((c) => (
                <th key={c.key} className={th}>
                  {crossed ? c.parts[1] : c.parts[0]}
                </th>
              ))}
          </tr>
        </thead>
        <tbody>
          {r.sections.map((s) => (
            <SectionRows key={s.id} m={m} sectionTitle={s.title} rows={s.rows} span={cols.length + 1} show={show} pos={pos} setPos={setPos} period={period} place={place} />
          ))}
        </tbody>
      </table>
    </div>
  )
}

function SectionRows({ m, sectionTitle, rows, span, show, pos, setPos, period, place }: { m: M; sectionTitle: string; rows: Row[]; span: number; show: 'values' | 'mapping'; pos: Pos; setPos: (p: Pos) => void; period: Period; place: string }) {
  return (
    <>
      <tr>
        <td colSpan={span} className={css({ fontWeight: 'semibold', px: '2', pt: '2', fontSize: 'sm' })}>
          {sectionTitle}
        </td>
      </tr>
      {rows.map((rw) => {
        const ind = indicator(rw.ind)
        if (!ind) return null
        return (
          <tr key={rw.id}>
            <td className={css({ border: '1px solid', borderColor: 'border', px: '2', py: '1', fontSize: 'sm' })}>{rw.label}</td>
            {columns(m.r, ind, period).map((c) => {
              const selected = pos?.row === rw.id && pos.col === c.key
              const list = m.r.mappings[cellKey(rw.id, c.key)]
              let text: string
              let tone = ''
              if (show === 'values') {
                const v = cell(ind, c, period, place)
                text = v.text
                if (v.why) tone = muted
              } else {
                const t = m.r.dhis2 ? mapText(list, TODAY) : null
                const needed = m.r.dhis2 && isCount(ind) && !c.total && !c.parts.includes('not recorded')
                text = t ?? (needed ? 'unmapped' : '·')
                tone = t ? mono : needed ? bad : muted
              }
              return (
                <td key={c.key} className={cx(td, tone, selected ? css({ outline: '2px solid', outlineColor: 'accent.default', outlineOffset: '-2px' }) : '')} onClick={() => setPos(selected ? null : { row: rw.id, col: c.key })}>
                  {text}
                </td>
              )
            })}
          </tr>
        )
      })}
    </>
  )
}

function MappingEditor({ m, rowId, col }: { m: M; rowId: string; col: string }) {
  const { r, set } = m
  const key = cellKey(rowId, col)
  const list = r.mappings[key] ?? []
  const put = (l: Mapping[]) => set({ ...r, mappings: { ...r.mappings, [key]: l } })
  const guess = COCS.find((c) => col.startsWith(c.label.slice(0, 2)))?.id ?? 'default'
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      {list.length === 0 ? <span className={muted}>Not sent. Add a mapping to send this cell.</span> : null}
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
          {x.to === '' ? <span className={muted}>(open)</span> : null}
          <button type="button" className={small} onClick={() => put(list.filter((_, j) => j !== i))}>
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
            put([...closed, { de: list.at(-1)?.de ?? 'AN01a', coc: list.at(-1)?.coc ?? guess, from: open === -1 ? '2024-07-01' : '2026-07-01', to: '' }])
          }}
        >
          {list.length === 0 ? '+ mapping' : '+ new mapping from a date (form changed)'}
        </button>
      </div>
    </div>
  )
}

function CellPanel({ m }: { m: M }) {
  const { pos, r, period, place } = m
  if (!pos) return <p className={muted}>Click a cell to see its mapping and who it counted.</p>
  const rw = r.sections.flatMap((s) => s.rows).find((x) => x.id === pos.row)
  const ind = rw ? indicator(rw.ind) : undefined
  const col = ind ? columns(r, ind, period).find((c: Col) => c.key === pos.col) : undefined
  if (!rw || !ind || !col) return null
  const v = cell(ind, col, period, place)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <h3 className={css({ fontWeight: 'semibold' })}>
        {rw.label} · {col.key}
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
          {period.label} · {PLACES.find((p) => p.id === place)?.label}
        </span>
      </div>
      {v.why ? <span className={muted}>{v.why}</span> : null}
      {v.patients.length > 0 ? (
        <ul className={cx(muted, css({ pl: '4', listStyle: 'disc' }))}>
          {v.patients.map((p) => (
            <li key={p.id}>
              {p.name} · {p.encounterType} · {p.dates.start} · {PLACES.find((x) => x.id === facilityOf(p))?.label}
            </li>
          ))}
        </ul>
      ) : null}
      {r.dhis2 ? (
        <>
          <h3 className={css({ fontWeight: 'semibold', mt: '2' })}>DHIS2</h3>
          {isCount(ind) ? <MappingEditor m={m} rowId={rw.id} col={col.key} /> : <span className={warn}>This row is a {ind.calc}; DHIS2 data elements on this form hold counts. Map the counts it is built from instead.</span>}
        </>
      ) : null}
    </div>
  )
}

function Issues({ m, tab }: { m: M; tab?: Issue['tab'] }) {
  const list = tab ? m.issues.filter((i) => i.tab === tab) : m.issues
  if (list.length === 0) return <p className={muted}>No issues.</p>
  return (
    <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
      {list.map((i, k) => (
        <li key={`${i.where}${k}`} className={i.level === 'error' ? bad : i.level === 'warning' ? warn : muted}>
          <strong>{i.where}:</strong> {i.text}
        </li>
      ))}
    </ul>
  )
}

const errors = (m: M, tab?: Issue['tab']) => m.issues.filter((i) => i.level === 'error' && (!tab || i.tab === tab)).length

// --- Variant A: the form is the editor --------------------------------------------------------------------

function FormVariant({ m }: { m: M }) {
  const [mode, setMode] = useState<'design' | 'preview'>('design')
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 24rem' }, gap: '4', alignItems: 'start' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <section className={cx(box, css({ p: '3' }))}>
          <Basics m={m} />
        </section>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '3' }))}>
          <div className={css({ display: 'flex', justifyContent: 'space-between', gap: '2', flexWrap: 'wrap' })}>
            <Columns m={m} />
            <div role="radiogroup" aria-label="Mode" className={row}>
              {(['design', 'preview'] as const).map((x) => (
                <label key={x}>
                  <input type="radio" checked={mode === x} onChange={() => setMode(x)} /> {x === 'design' ? 'Design (DHIS2 cells)' : 'Preview (values)'}
                </label>
              ))}
            </div>
          </div>
          {mode === 'preview' ? <PreviewControls m={m} /> : null}
          <Table m={m} show={mode === 'preview' ? 'values' : 'mapping'} />
          <details>
            <summary className={css({ cursor: 'pointer', fontSize: 'sm' })}>Edit sections and rows</summary>
            <div className={css({ pt: '2' })}>
              <Outline m={m} />
            </div>
          </details>
        </section>
      </div>
      <aside className={css({ display: 'flex', flexDirection: 'column', gap: '3', position: { lg: 'sticky' }, top: '4' })}>
        <section className={cx(box, css({ p: '3' }))}>
          <CellPanel m={m} />
        </section>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
          <h2 className={h2}>Validation ({errors(m)} errors)</h2>
          <Issues m={m} />
        </section>
      </aside>
    </div>
  )
}

// --- Variant B: outline and live preview, mapping as one sheet ---------------------------------------------

function MappingSheet({ m }: { m: M }) {
  const { r } = m
  if (!r.dhis2) return <p className={muted}>Not sent to DHIS2. Tick “Sent to DHIS2” to map cells.</p>
  const lines = r.sections.flatMap((s) =>
    s.rows.flatMap((rw) => {
      const ind = indicator(rw.ind)
      if (!ind) return []
      return columns(r, ind, m.period).map((c) => ({ rw, ind, c }))
    }),
  )
  return (
    <table className={css({ borderCollapse: 'collapse', width: '100%', fontSize: 'sm' })}>
      <thead>
        <tr>
          {['Row', 'Column', 'In effect today', 'Mappings'].map((h) => (
            <th key={h} className={cx(th, css({ textAlign: 'left' }))}>
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {lines.map(({ rw, ind, c }) => {
          const list = r.mappings[cellKey(rw.id, c.key)]
          const t = mapText(list, TODAY)
          const needed = isCount(ind) && !c.total && !c.parts.includes('not recorded')
          return (
            <tr key={`${rw.id}${c.key}`}>
              <td className={css({ border: '1px solid', borderColor: 'border', px: '2' })}>{rw.label}</td>
              <td className={css({ border: '1px solid', borderColor: 'border', px: '2' })}>{c.key}</td>
              <td className={cx(css({ border: '1px solid', borderColor: 'border', px: '2' }), t ? mono : needed ? bad : muted)}>{t ?? (!isCount(ind) ? `a ${ind.calc}, not sent` : needed ? 'unmapped' : 'not sent')}</td>
              <td className={css({ border: '1px solid', borderColor: 'border', px: '2', py: '1' })}>{isCount(ind) ? <MappingEditor m={m} rowId={rw.id} col={c.key} /> : null}</td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}

function OutlineVariant({ m }: { m: M }) {
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '28rem 1fr' }, gap: '4', alignItems: 'start' })}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <section className={cx(box, css({ p: '3' }))}>
            <Basics m={m} />
          </section>
          <section className={cx(box, css({ p: '3' }))}>
            <Columns m={m} />
          </section>
          <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h2 className={h2}>Sections and rows</h2>
            <Outline m={m} />
          </section>
          <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h2 className={h2}>Validation ({errors(m)} errors)</h2>
            <Issues m={m} />
          </section>
        </div>
        <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '3', position: { lg: 'sticky' }, top: '4' }))}>
          <PreviewControls m={m} />
          <h2 className={h2}>{m.r.title}</h2>
          <Table m={m} show="values" />
          <CellPanel m={m} />
        </section>
      </div>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <h2 className={h2}>DHIS2 mapping</h2>
        <MappingSheet m={m} />
      </section>
    </div>
  )
}

// --- Variant C: steps -----------------------------------------------------------------------------------

const STEPS: { id: Issue['tab'] | 'preview'; label: string }[] = [
  { id: 'basics', label: '1 Basics' },
  { id: 'rows', label: '2 Rows' },
  { id: 'columns', label: '3 Columns' },
  { id: 'dhis2', label: '4 DHIS2' },
  { id: 'preview', label: '5 Preview' },
]
function StepsVariant({ m }: { m: M }) {
  const [step, setStep] = useState<(typeof STEPS)[number]['id']>('basics')
  const i = STEPS.findIndex((s) => s.id === step)
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 20rem' }, gap: '4', alignItems: 'start' })}>
      <section className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '3' }))}>
        <nav className={css({ display: 'flex', gap: '1', flexWrap: 'wrap' })}>
          {STEPS.map((s) => {
            const n = s.id === 'preview' ? 0 : errors(m, s.id)
            return (
              <button key={s.id} type="button" className={cx(ghost, s.id === step ? css({ bg: 'bg.subtle', fontWeight: 'semibold' }) : '')} onClick={() => setStep(s.id)}>
                {s.label}
                {n > 0 ? <span className={bad}> ({n})</span> : null}
              </button>
            )
          })}
        </nav>
        {step === 'basics' ? <Basics m={m} /> : null}
        {step === 'rows' ? <Outline m={m} /> : null}
        {step === 'columns' ? (
          <>
            <Columns m={m} />
            <Table m={m} show="mapping" compact />
          </>
        ) : null}
        {step === 'dhis2' ? (
          <>
            <p className={muted}>Click a cell to map it. Red cells are counts with no mapping in effect today.</p>
            <Table m={m} show="mapping" />
            {m.pos && m.r.dhis2 ? <MappingEditor m={m} rowId={m.pos.row} col={m.pos.col} /> : null}
          </>
        ) : null}
        {step === 'preview' ? (
          <>
            <PreviewControls m={m} />
            <Table m={m} show="values" />
            <CellPanel m={m} />
            <div className={row}>
              <button type="button" className={ghost}>
                Print / PDF
              </button>
              <button type="button" className={ghost}>
                CSV
              </button>
            </div>
          </>
        ) : null}
        <div className={css({ display: 'flex', justifyContent: 'space-between' })}>
          <button type="button" className={ghost} disabled={i === 0} onClick={() => setStep(STEPS[i - 1]?.id ?? step)}>
            Back
          </button>
          <button type="button" className={ghost} disabled={i === STEPS.length - 1} onClick={() => setStep(STEPS[i + 1]?.id ?? step)}>
            Next
          </button>
        </div>
      </section>
      <aside className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
        <h2 className={h2}>Issues</h2>
        <Issues m={m} />
      </aside>
    </div>
  )
}

// --- Shell ---------------------------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'the form is the editor', View: FormVariant },
  { key: 'B', name: 'outline and live preview', View: OutlineVariant },
  { key: 'C', name: 'steps', View: StepsVariant },
]

export function ReportDesignerPrototype() {
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
