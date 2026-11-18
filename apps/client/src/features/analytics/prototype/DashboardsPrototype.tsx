// PROTOTYPE (#597), throwaway: the data explorer and Dashboards on /prototype/dashboards, three layouts
// switched with ?variant=A|B|C and the floating bar (← / → keys). Made-up summaries (dashboardData.ts);
// the role switcher changes the user's area and whether they are on central or a facility's edge.
// A: explorer first. A DHIS2-style data explorer (pick data, periods, places, disaggregation, chart
//    type) that saves Visualizations; Dashboards are a second tab.
// B: dashboard first. A Dashboard with a filter bar over every item; each item opens an explorer in a
//    drawer, and "Add" walks through What, Where, When and Show as.
// C: place scorecard. Pick a place and period; indicators × child places, coloured by threshold, with
//    sparklines and a map; click a place to drill down, pin a row to a Dashboard.
import { css, cx } from '@health/design-system/css'
import { useEffect, useState, type ReactNode } from 'react'

import {
  CURRENT,
  DISAGGS,
  INDICATORS,
  INITIAL_DASHBOARDS,
  INITIAL_VIS,
  PERIODS,
  PERIOD_TYPE_LABEL,
  ROLES,
  SYNCED,
  childrenOf,
  facilitiesUnder,
  indOf,
  pathTo,
  placeOf,
  range,
  show,
  toneOf,
  value,
  within,
  type ChartType,
  type Dashboard,
  type Ind,
  type Period,
  type PeriodType,
  type Place,
  type Role,
  type Share,
  type Vis,
} from './dashboardData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const primary = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', bg: 'accent.default', color: 'accent.fg', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const small = cx(ghost, css({ minHeight: 'auto', py: '0.5', px: '2', fontSize: 'xs' }))
const on = css({ bg: 'bg.selected', color: 'fg.selected', borderColor: 'accent.default' })
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const h3 = css({ fontSize: 'sm', fontWeight: 'semibold' })
const row = css({ display: 'flex', gap: '2', alignItems: 'center', flexWrap: 'wrap' })
const chip = css({ fontSize: 'xs', px: '1.5', borderRadius: 'sm', border: '1px solid', borderColor: 'border', whiteSpace: 'nowrap' })
const field = css({ border: '1px solid', borderColor: 'border.control', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface', fontSize: 'sm' })
const TONE = {
  good: css({ bg: 'green.100', color: 'green.900' }),
  warn: css({ bg: 'amber.100', color: 'amber.900' }),
  bad: css({ bg: 'red.100', color: 'red.900' }),
}
const SERIES_COLORS = ['#1d4ed8', '#c2410c', '#15803d', '#7e22ce', '#0e7490', '#a16207', '#be123c', '#4d7c0f']
const TONE_FILL = { good: '#86efac', warn: '#fcd34d', bad: '#fca5a5' }

// --- Model ------------------------------------------------------------------------------------------------

type M = {
  role: Role
  setRole: (id: string) => void
  vis: Vis[]
  saveVis: (v: Vis, dashboard: string | null) => void
  dashboards: Dashboard[]
  setDashboards: (f: (d: Dashboard[]) => Dashboard[]) => void
}
let seq = 100
const nextId = (p: string) => `${p}${(seq += 1)}`

function useModel(): M {
  const [roleId, setRole] = useState('dho')
  const [vis, setVis] = useState<Vis[]>(INITIAL_VIS)
  const [dashboards, setDashboards] = useState<Dashboard[]>(INITIAL_DASHBOARDS)
  const role = ROLES.find((r) => r.id === roleId) ?? (ROLES[0] as Role)
  return {
    role,
    setRole,
    vis,
    dashboards,
    setDashboards,
    saveVis: (v, dashboard) => {
      setVis((all) => (all.some((x) => x.id === v.id) ? all.map((x) => (x.id === v.id ? v : x)) : [...all, v]))
      if (dashboard) setDashboards((ds) => ds.map((d) => (d.id === dashboard && !d.items.includes(v.id) ? { ...d, items: [...d.items, v.id] } : d)))
    },
  }
}
const visOf = (m: M, id: string) => m.vis.find((v) => v.id === id)
const blank = (root: string): Vis => ({ id: nextId('v'), title: 'Untitled Visualization', type: 'column', inds: ['anc1-cov'], places: [root], children: false, periodType: 'month', from: '2025-10', to: CURRENT, series: 'ind', disagg: 'none' })

// --- Resolving a Visualization ------------------------------------------------------------------------------

function placesFor(v: Vis, root: string): { list: Place[]; outside: string[] } {
  const rootPlace = placeOf(root) as Place
  if (v.children) {
    const kids = childrenOf(root)
    return { list: kids.length ? kids : [rootPlace], outside: [] }
  }
  const asked = v.places.length ? v.places : [root]
  const list = asked.filter((id) => within(root, id)).map((id) => placeOf(id) as Place)
  return { list: list.length ? list : [rootPlace], outside: asked.filter((id) => !within(root, id)).map((id) => placeOf(id)?.label ?? id) }
}
const monthsOf = (ps: Period[]) => ps.flatMap((p) => p.months)

type Series = { label: string; values: (number | null)[]; ind: Ind }
type Shape = { categories: string[]; series: Series[] }
function shape(v: Vis, places: Place[]): Shape {
  const periods = range(v.periodType, v.from, v.to)
  const inds = v.inds.map(indOf)
  const firstInd = inds[0] ?? (INDICATORS[0] as Ind)
  const firstPlace = places[0] as Place
  const byPeriods = periods.length > 1 || v.series !== 'place'
  if (!byPeriods) return { categories: places.map((p) => p.label), series: inds.map((i) => ({ label: i.short, ind: i, values: places.map((p) => value(i, p.id, monthsOf(periods)).v) })) }
  const categories = periods.map((p) => p.label)
  if (v.series === 'place') return { categories, series: places.map((p) => ({ label: p.label, ind: firstInd, values: periods.map((pe) => value(firstInd, p.id, pe.months).v) })) }
  if (v.series === 'disagg') {
    const d = DISAGGS.find((x) => x.id === v.disagg)
    if (d && d.bands.length) {
      return {
        categories,
        series: d.bands.map((b, k) => ({
          label: b.label,
          ind: firstInd,
          values: periods.map((pe) => {
            const x = value(firstInd, firstPlace.id, pe.months).v
            if (x === null) return null
            return firstInd.calc === 'count' ? x * b.share : x * (0.85 + 0.1 * k)
          }),
        })),
      }
    }
  }
  return { categories, series: inds.map((i) => ({ label: i.short, ind: i, values: periods.map((pe) => value(i, firstPlace.id, pe.months).v) })) }
}

// --- Charts -------------------------------------------------------------------------------------------------

function Bars({ s, line }: { s: Shape; line: boolean }) {
  const W = 640
  const H = 220
  const L = 44
  const B = 36
  const all = s.series.flatMap((x) => x.values).filter((x): x is number => x !== null)
  const max = Math.max(1, ...all) * 1.1
  const cw = (W - L - 8) / Math.max(1, s.categories.length)
  const y = (val: number) => H - B - ((H - B - 8) * val) / max
  const bw = Math.max(3, (cw * 0.8) / Math.max(1, s.series.length))
  const pct = s.series.every((x) => x.ind.calc !== 'count')
  return (
    <div>
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label="chart" className={css({ width: '100%', height: 'auto' })}>
        {[0, 0.5, 1].map((t) => (
          <g key={t}>
            <line x1={L} x2={W} y1={y(max * t)} y2={y(max * t)} stroke="#e5e7eb" />
            <text x={L - 4} y={y(max * t) + 4} fontSize="10" textAnchor="end" fill="#6b7280">
              {pct ? `${Math.round(max * t)}%` : Math.round(max * t).toLocaleString('en-GB')}
            </text>
          </g>
        ))}
        {s.categories.map((c, i) => (
          <text key={c} x={L + cw * i + cw / 2} y={H - B + 14} fontSize="10" textAnchor="middle" fill="#374151">
            {c.length > 14 ? `${c.slice(0, 13)}…` : c}
          </text>
        ))}
        {s.series.map((se, k) =>
          line ? (
            <g key={se.label}>
              <polyline
                fill="none"
                stroke={SERIES_COLORS[k % SERIES_COLORS.length]}
                strokeWidth="2"
                points={se.values
                  .map((val, i) => (val === null ? null : `${L + cw * i + cw / 2},${y(val)}`))
                  .filter(Boolean)
                  .join(' ')}
              />
              {se.values.map((val, i) =>
                val === null ? (
                  <text key={i} x={L + cw * i + cw / 2} y={H - B - 4} fontSize="10" textAnchor="middle" fill="#b91c1c">
                    missing
                  </text>
                ) : (
                  <circle key={i} cx={L + cw * i + cw / 2} cy={y(val)} r="3" fill={SERIES_COLORS[k % SERIES_COLORS.length]}>
                    <title>{`${se.label}, ${s.categories[i]}: ${show(se.ind, val)}`}</title>
                  </circle>
                ),
              )}
            </g>
          ) : (
            <g key={se.label}>
              {se.values.map((val, i) => {
                const x = L + cw * i + cw * 0.1 + bw * k
                if (val === null)
                  return (
                    <text key={i} x={x + bw / 2} y={H - B - 4} fontSize="9" textAnchor="middle" fill="#b91c1c">
                      —
                    </text>
                  )
                const tone = toneOf(se.ind, val)
                return (
                  <rect key={i} x={x} y={y(val)} width={bw - 1} height={H - B - y(val)} fill={s.series.length === 1 && tone ? TONE_FILL[tone] : SERIES_COLORS[k % SERIES_COLORS.length]}>
                    <title>{`${se.label}, ${s.categories[i]}: ${show(se.ind, val)}`}</title>
                  </rect>
                )
              })}
            </g>
          ),
        )}
      </svg>
      {s.series.length > 1 ? (
        <div className={row}>
          {s.series.map((se, k) => (
            <span key={se.label} className={css({ fontSize: 'xs', display: 'inline-flex', gap: '1', alignItems: 'center' })}>
              <span style={{ width: 10, height: 10, background: SERIES_COLORS[k % SERIES_COLORS.length], display: 'inline-block' }} />
              {se.label}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  )
}

function Cell({ ind, v }: { ind: Ind; v: number | null }) {
  const tone = toneOf(ind, v)
  return <td className={cx(css({ px: '2', py: '1', textAlign: 'right', borderTop: '1px solid', borderColor: 'border' }), tone ? TONE[tone] : undefined)}>{show(ind, v)}</td>
}

function Table({ v, places }: { v: Vis; places: Place[] }) {
  const periods = range(v.periodType, v.from, v.to)
  const inds = v.inds.map(indOf)
  const th = css({ px: '2', py: '1', textAlign: 'right', fontWeight: 'semibold', fontSize: 'xs' })
  if (v.series === 'place' || periods.length === 1)
    return (
      <table className={css({ width: '100%', fontSize: 'sm', borderCollapse: 'collapse' })}>
        <thead>
          <tr>
            <th className={cx(th, css({ textAlign: 'left' }))}>{periods.map((p) => p.label).join(', ')}</th>
            {inds.map((i) => (
              <th key={i.id} className={th} title={i.title}>
                {i.short}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {places.map((p) => (
            <tr key={p.id}>
              <td className={css({ px: '2', py: '1', borderTop: '1px solid', borderColor: 'border' })}>{p.label}</td>
              {inds.map((i) => (
                <Cell key={i.id} ind={i} v={value(i, p.id, monthsOf(periods)).v} />
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    )
  const place = places[0] as Place
  return (
    <table className={css({ width: '100%', fontSize: 'sm', borderCollapse: 'collapse' })}>
      <thead>
        <tr>
          <th className={cx(th, css({ textAlign: 'left' }))}>{place.label}</th>
          {periods.map((p) => (
            <th key={p.id} className={th}>
              {p.label}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {inds.map((i) => (
          <tr key={i.id}>
            <td className={css({ px: '2', py: '1', borderTop: '1px solid', borderColor: 'border' })}>{i.short}</td>
            {periods.map((p) => (
              <Cell key={p.id} ind={i} v={value(i, place.id, p.months).v} />
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function Single({ v, places }: { v: Vis; places: Place[] }) {
  const periods = range(v.periodType, v.from, v.to)
  const ind = indOf(v.inds[0] ?? 'anc1-cov')
  const place = places[0] as Place
  const now = value(ind, place.id, monthsOf(periods)).v
  const list = PERIODS[v.periodType]
  const before = list[list.findIndex((p) => p.id === periods[0]?.id) - 1]
  const prev = before ? value(ind, place.id, before.months).v : null
  const tone = toneOf(ind, now)
  return (
    <div className={cx(css({ p: '4', borderRadius: 'md', textAlign: 'center' }), tone ? TONE[tone] : undefined)}>
      <div className={css({ fontSize: '4xl', fontWeight: 'bold' })}>{show(ind, now)}</div>
      <div className={css({ fontSize: 'sm' })}>
        {ind.title}, {place.label}
      </div>
      {now !== null && prev !== null ? (
        <div className={css({ fontSize: 'xs' })}>
          {now >= prev ? '▲' : '▼'} {ind.calc === 'count' ? show(ind, Math.abs(now - prev)) : `${Math.abs(now - prev).toFixed(1)} points`} on {before?.label}
        </div>
      ) : null}
    </div>
  )
}

function MapView({ v, places, onPick }: { v: Vis; places: Place[]; onPick?: (id: string) => void }) {
  const ind = indOf(v.inds[0] ?? 'malaria-pos')
  const months = monthsOf(range(v.periodType, v.from, v.to))
  const facilities = places.flatMap((p) => facilitiesUnder(p.id)).map((id) => placeOf(id) as Place)
  return (
    <div>
      <svg viewBox="0 0 100 90" role="img" aria-label="map" className={css({ width: '100%', height: 'auto', maxHeight: '260px', bg: 'bg.subtle', borderRadius: 'md' })}>
        <path d="M30,5 L75,8 L88,40 L82,80 L35,85 L15,55 Z" fill="#f9fafb" stroke="#9ca3af" strokeWidth="0.4" />
        {places
          .filter((p) => p.level !== 'facility')
          .map((p) => {
            const val = value(ind, p.id, months).v
            const tone = toneOf(ind, val)
            return (
              <g key={p.id} onClick={() => onPick?.(p.id)} className={css({ cursor: onPick ? 'pointer' : 'default' })}>
                <circle cx={p.x} cy={p.y} r={p.level === 'region' ? 16 : 8} fill={tone ? TONE_FILL[tone] : '#e5e7eb'} stroke="#6b7280" strokeWidth="0.3" opacity="0.85">
                  <title>{`${p.label}: ${show(ind, val)}`}</title>
                </circle>
                <text x={p.x} y={p.y - (p.level === 'region' ? 17 : 9)} fontSize="3" textAnchor="middle">
                  {p.label}
                </text>
              </g>
            )
          })}
        {facilities.map((f) => {
          const val = value(ind, f.id, months).v
          const tone = toneOf(ind, val)
          const asked = places.every((p) => p.level === 'facility')
          return (
            <g key={f.id}>
              <circle cx={f.x} cy={f.y} r={asked ? 2.4 : 1.3} fill={val === null ? '#ffffff' : asked && tone ? TONE_FILL[tone] : '#111827'} stroke="#111827" strokeWidth="0.4">
                <title>{`${f.label}: ${val === null ? 'not synced' : show(ind, val)}`}</title>
              </circle>
              {asked ? (
                <text x={f.x + 3} y={f.y + 1} fontSize="2.6">
                  {f.label}: {val === null ? 'not synced' : show(ind, val)}
                </text>
              ) : null}
            </g>
          )
        })}
      </svg>
      <div className={cx(row, muted)}>
        Schematic: boundaries ship in the national core package; facility points from the NHFR. ● facility, ○ not synced.
        {ind.bands ? <Legend ind={ind} /> : null}
      </div>
    </div>
  )
}

function Legend({ ind }: { ind: Ind }) {
  let lo = 0
  return (
    <span className={row}>
      {ind.bands?.map((b) => {
        const label = b.below === Infinity ? `≥ ${lo}` : `${lo}–${b.below}`
        lo = b.below
        return (
          <span key={label} className={cx(chip, TONE[b.tone])}>
            {label}
          </span>
        )
      })}
    </span>
  )
}

// --- A Visualization card: freshness, completeness, scope -----------------------------------------------------

function Freshness({ v, places, m }: { v: Vis; places: Place[]; m: M }) {
  const months = monthsOf(range(v.periodType, v.from, v.to))
  const facs = [...new Set(places.flatMap((p) => facilitiesUnder(p.id)))]
  const ind = indOf(v.inds[0] ?? 'anc1-cov')
  const missing = facs.filter((f) => !value(ind, f, months).reported)
  const latest = facs.map((f) => SYNCED[f] ?? '').reduce((a, b) => (b > a ? b : a), '')
  const medianAbove = v.inds.map(indOf).some((i) => i.calc === 'median') && places.some((p) => p.level !== 'facility')
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'xs', color: 'fg.muted' })}>
      <span>
        {m.role.node === 'edge' ? 'Computed on this facility' : 'Rolled up at central'} · as of {latest} · {facs.length - missing.length}/{facs.length} facilities reported
      </span>
      {missing.length ? (
        <span className={css({ color: 'danger.text' })}>
          Missing, not zero: {missing.map((f) => `${placeOf(f)?.label} (last synced ${SYNCED[f]?.slice(0, 10)})`).join('; ')}
        </span>
      ) : null}
      {medianAbove ? <span>Median gestational age shows at facility level only: a median can't be rebuilt from facility summaries.</span> : null}
    </div>
  )
}

function VisCard({ v, m, actions, root }: { v: Vis; m: M; actions?: ReactNode; root?: string }) {
  const { list, outside } = placesFor(v, root ?? m.role.root)
  const s = shape(v, list)
  return (
    <div className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2', minWidth: 0 }))}>
      <div className={css({ display: 'flex', justifyContent: 'space-between', gap: '2', alignItems: 'start' })}>
        <div>
          <div className={h3}>{v.title}</div>
          <div className={muted}>
            {PERIOD_TYPE_LABEL[v.periodType]}: {range(v.periodType, v.from, v.to).map((p) => p.label).join(', ').slice(0, 60)} · {list.map((p) => p.label).join(', ')}
          </div>
        </div>
        <div className={row}>{actions}</div>
      </div>
      {outside.length ? <div className={css({ fontSize: 'xs', color: 'danger.text' })}>Outside your area, not shown: {outside.join(', ')}</div> : null}
      {v.type === 'table' ? <Table v={v} places={list} /> : v.type === 'single' ? <Single v={v} places={list} /> : v.type === 'map' ? <MapView v={v} places={list} /> : <Bars s={s} line={v.type === 'line'} />}
      <Freshness v={v} places={list} m={m} />
    </div>
  )
}

// --- The explorer (shared by A and B) ---------------------------------------------------------------------------

const TYPES: { id: ChartType; label: string }[] = [
  { id: 'column', label: '▮ Column' },
  { id: 'line', label: '⟋ Line' },
  { id: 'table', label: '▦ Table' },
  { id: 'single', label: '# Single' },
  { id: 'map', label: '◍ Map' },
]

function PeriodPicker({ v, set }: { v: Vis; set: (p: Partial<Vis>) => void }) {
  const list = PERIODS[v.periodType]
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <div className={row}>
        {(Object.keys(PERIODS) as PeriodType[]).map((t) => (
          <button key={t} type="button" className={cx(small, v.periodType === t && on)} onClick={() => set({ periodType: t, from: PERIODS[t][0]?.id ?? '', to: PERIODS[t].at(-1)?.id ?? '' })}>
            {PERIOD_TYPE_LABEL[t]}
          </button>
        ))}
      </div>
      <div className={row}>
        <select aria-label="From" className={field} value={v.from} onChange={(e) => set({ from: e.target.value })}>
          {list.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
        to
        <select aria-label="To" className={field} value={v.to} onChange={(e) => set({ to: e.target.value })}>
          {list.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
      </div>
      <span className={muted}>Standard periods only: no arbitrary dates.</span>
    </div>
  )
}

function PlacePicker({ v, set, root }: { v: Vis; set: (p: Partial<Vis>) => void; root: string }) {
  const tree = (id: string, depth: number): ReactNode => {
    const p = placeOf(id) as Place
    const checked = v.places.includes(id)
    return (
      <div key={id}>
        <label className={css({ display: 'flex', gap: '1', fontSize: 'sm', alignItems: 'center' })} style={{ paddingLeft: depth * 12 }}>
          <input type="checkbox" disabled={v.children} checked={checked} onChange={() => set({ places: checked ? v.places.filter((x) => x !== id) : [...v.places, id] })} />
          {p.label}
        </label>
        {childrenOf(id).map((c) => tree(c.id, depth + 1))}
      </div>
    )
  }
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <label className={css({ display: 'flex', gap: '1', fontSize: 'sm' })}>
        <input type="checkbox" checked={v.children} onChange={() => set({ children: !v.children })} />
        Each area under my place (follows whoever views it)
      </label>
      <div className={css({ maxHeight: '160px', overflowY: 'auto' })}>{tree(root, 0)}</div>
      <span className={muted}>Only places in your area: {placeOf(root)?.label}.</span>
    </div>
  )
}

function Explorer({ v, set, root, compact }: { v: Vis; set: (p: Partial<Vis>) => void; root: string; compact?: boolean }) {
  const section = css({ display: 'flex', flexDirection: 'column', gap: '1', pb: '2', borderBottom: '1px solid', borderColor: 'border' })
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' })}>
      <div className={section}>
        <span className={h3}>Data</span>
        {INDICATORS.map((i) => (
          <label key={i.id} className={css({ display: 'flex', gap: '1' })}>
            <input type="checkbox" checked={v.inds.includes(i.id)} onChange={() => set({ inds: v.inds.includes(i.id) ? v.inds.filter((x) => x !== i.id) : [...v.inds, i.id] })} />
            {i.title}
            {i.calc === 'median' ? <span className={chip}>facility only</span> : null}
          </label>
        ))}
      </div>
      <div className={section}>
        <span className={h3}>Periods</span>
        <PeriodPicker v={v} set={set} />
      </div>
      <div className={section}>
        <span className={h3}>Places</span>
        <PlacePicker v={v} set={set} root={root} />
      </div>
      {compact ? null : (
        <div className={section}>
          <span className={h3}>Disaggregation</span>
          <select aria-label="Disaggregation" className={field} value={v.disagg} onChange={(e) => set({ disagg: e.target.value, series: e.target.value === 'none' ? 'ind' : 'disagg' })}>
            {DISAGGS.map((d) => (
              <option key={d.id} value={d.id}>
                {d.label}
              </option>
            ))}
          </select>
        </div>
      )}
      <div className={section}>
        <span className={h3}>Series</span>
        <div className={row}>
          {(['ind', 'place', 'disagg'] as const).map((s) => (
            <button key={s} type="button" disabled={s === 'disagg' && v.disagg === 'none'} className={cx(small, v.series === s && on)} onClick={() => set({ series: s })}>
              {s === 'ind' ? 'Indicators' : s === 'place' ? 'Places' : 'Disaggregation'}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

function TypeBar({ v, set }: { v: Vis; set: (p: Partial<Vis>) => void }) {
  return (
    <div className={row} role="group" aria-label="Show as">
      {TYPES.map((t) => (
        <button key={t.id} type="button" className={cx(small, v.type === t.id && on)} onClick={() => set({ type: t.id })}>
          {t.label}
        </button>
      ))}
    </div>
  )
}

function SaveBar({ v, set, m, onSaved }: { v: Vis; set: (p: Partial<Vis>) => void; m: M; onSaved?: () => void }) {
  const mine = m.dashboards.filter((d) => d.kind === 'saved')
  const [target, setTarget] = useState(mine[0]?.id ?? '')
  if (!m.role.author) return <div className={muted}>Saving needs analytics.author. You can explore, and open the Dashboards shared with you.</div>
  return (
    <div className={row}>
      <input aria-label="Title" className={cx(field, css({ minWidth: '240px' }))} value={v.title} onChange={(e) => set({ title: e.target.value })} />
      <select aria-label="Add to Dashboard" className={field} value={target} onChange={(e) => setTarget(e.target.value)}>
        <option value="">Don't add to a Dashboard</option>
        {mine.map((d) => (
          <option key={d.id} value={d.id}>
            Add to {d.title}
          </option>
        ))}
      </select>
      <button
        type="button"
        className={primary}
        onClick={() => {
          m.saveVis(v, target || null)
          onSaved?.()
        }}
      >
        Save Visualization
      </button>
    </div>
  )
}

function ShareEditor({ d, m }: { d: Dashboard; m: M }) {
  const [kind, setKind] = useState<Share['kind']>('role')
  const [label, setLabel] = useState('')
  if (d.kind === 'standard') return <div className={muted}>Standard Dashboard from {d.source}: everyone sees it; change it through a new package version.</div>
  const update = (shared: Share[]) => m.setDashboards((ds) => ds.map((x) => (x.id === d.id ? { ...x, shared } : x)))
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <div className={row}>
        {d.shared.map((s) => (
          <span key={s.label} className={chip}>
            {s.kind}: {s.label}{' '}
            {m.role.author ? (
              <button type="button" aria-label={`Stop sharing with ${s.label}`} onClick={() => update(d.shared.filter((x) => x !== s))}>
                ×
              </button>
            ) : null}
          </span>
        ))}
      </div>
      {m.role.author ? (
        <div className={row}>
          <select aria-label="Share with" className={field} value={kind} onChange={(e) => setKind(e.target.value as Share['kind'])}>
            <option value="person">A person</option>
            <option value="role">A role</option>
            <option value="level">A place level</option>
          </select>
          <input aria-label="Who" className={field} placeholder={kind === 'level' ? 'e.g. every district' : kind === 'role' ? 'e.g. Facility in-charges' : 'Name'} value={label} onChange={(e) => setLabel(e.target.value)} />
          <button type="button" className={small} disabled={!label} onClick={() => (update([...d.shared, { kind, label }]), setLabel(''))}>
            Share
          </button>
          <span className={muted}>Each viewer still sees only their own area.</span>
        </div>
      ) : null}
    </div>
  )
}

// --- Variant A: explorer first -----------------------------------------------------------------------------------

function ExplorerFirst({ m }: { m: M }) {
  const [tab, setTab] = useState<'explore' | 'dashboards'>('explore')
  const [v, setV] = useState<Vis>(() => ({ ...(INITIAL_VIS[0] as Vis), id: nextId('v'), title: 'ANC coverage by month (copy)' }))
  const [dash, setDash] = useState(m.dashboards[0]?.id ?? '')
  const set = (p: Partial<Vis>) => setV((x) => ({ ...x, ...p }))
  const d = m.dashboards.find((x) => x.id === dash)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <div className={row} role="tablist">
        <button type="button" role="tab" aria-selected={tab === 'explore'} className={cx(ghost, tab === 'explore' && on)} onClick={() => setTab('explore')}>
          Data explorer
        </button>
        <button type="button" role="tab" aria-selected={tab === 'dashboards'} className={cx(ghost, tab === 'dashboards' && on)} onClick={() => setTab('dashboards')}>
          Dashboards
        </button>
      </div>
      {tab === 'explore' ? (
        <div className={css({ display: 'grid', gridTemplateColumns: '300px 1fr', gap: '3', alignItems: 'start' })}>
          <div className={cx(box, css({ p: '3' }))}>
            <Explorer v={v} set={set} root={m.role.root} />
          </div>
          <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
            <TypeBar v={v} set={set} />
            <VisCard v={v} m={m} />
            <SaveBar v={v} set={set} m={m} onSaved={() => setV((x) => ({ ...x, id: nextId('v') }))} />
          </div>
        </div>
      ) : (
        <div className={css({ display: 'grid', gridTemplateColumns: '240px 1fr', gap: '3', alignItems: 'start' })}>
          <div className={cx(box, css({ p: '2', display: 'flex', flexDirection: 'column', gap: '1' }))}>
            {m.dashboards.map((x) => (
              <button key={x.id} type="button" className={cx(ghost, css({ textAlign: 'left' }), x.id === dash && on)} onClick={() => setDash(x.id)}>
                {x.title}
                <div className={muted}>{x.kind === 'standard' ? 'Standard' : 'Saved'}</div>
              </button>
            ))}
            {m.role.author ? (
              <button
                type="button"
                className={small}
                onClick={() => {
                  const id = nextId('d')
                  m.setDashboards((ds) => [...ds, { id, title: 'New Dashboard', kind: 'saved', source: `Saved by ${m.role.label}`, shared: [], items: [] }])
                  setDash(id)
                }}
              >
                + New Dashboard
              </button>
            ) : null}
          </div>
          {d ? (
            <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
              <div className={h2}>{d.title}</div>
              <div className={muted}>{d.source}</div>
              <ShareEditor d={d} m={m} />
              <div className={css({ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(420px, 1fr))', gap: '3' })}>
                {d.items.map((id) => {
                  const x = visOf(m, id)
                  return x ? (
                    <VisCard
                      key={id}
                      v={x}
                      m={m}
                      actions={
                        <button type="button" className={small} onClick={() => (setV({ ...x }), setTab('explore'))}>
                          Open in explorer
                        </button>
                      }
                    />
                  ) : null
                })}
                {d.items.length === 0 ? <div className={muted}>Empty. Save a Visualization from the explorer into it.</div> : null}
              </div>
            </div>
          ) : null}
        </div>
      )}
    </div>
  )
}

// --- Variant B: dashboard first -----------------------------------------------------------------------------------

function DashboardFirst({ m }: { m: M }) {
  const [dash, setDash] = useState(m.dashboards[0]?.id ?? '')
  const [override, setOverride] = useState<{ periodType: PeriodType; from: string; to: string } | null>(null)
  const [place, setPlace] = useState<string | null>(null)
  const [editing, setEditing] = useState<Vis | null>(null)
  const [adding, setAdding] = useState<{ step: number; v: Vis } | null>(null)
  const [sharing, setSharing] = useState(false)
  const d = m.dashboards.find((x) => x.id === dash)
  const root = place ?? m.role.root
  const apply = (v: Vis): Vis => (override ? { ...v, ...override } : v)
  const editable = d?.kind === 'saved' && m.role.author
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <div className={row}>
        {m.dashboards.map((x) => (
          <button key={x.id} type="button" className={cx(ghost, x.id === dash && on)} onClick={() => setDash(x.id)}>
            {x.title} <span className={chip}>{x.kind}</span>
          </button>
        ))}
        {m.role.author ? (
          <button
            type="button"
            className={small}
            onClick={() => {
              const id = nextId('d')
              m.setDashboards((ds) => [...ds, { id, title: 'New Dashboard', kind: 'saved', source: `Saved by ${m.role.label}`, shared: [], items: [] }])
              setDash(id)
            }}
          >
            + New Dashboard
          </button>
        ) : null}
      </div>
      {d ? (
        <>
          <div className={cx(box, css({ p: '2', display: 'flex', gap: '3', alignItems: 'center', flexWrap: 'wrap' }))}>
            <span className={h3}>Filter every item</span>
            <label className={row}>
              <input type="checkbox" checked={override !== null} onChange={() => setOverride(override ? null : { periodType: 'quarter', from: '2026Q1', to: '2026Q1' })} />
              Same periods for all
            </label>
            {override ? <PeriodPicker v={{ ...(INITIAL_VIS[0] as Vis), ...override }} set={(p) => setOverride((o) => (o ? { ...o, ...p } : o))} /> : <span className={muted}>Each item keeps its own periods</span>}
            <label className={row}>
              Place
              <select aria-label="Place" className={field} value={root} onChange={(e) => setPlace(e.target.value)}>
                {[m.role.root, ...facilitiesUnder(m.role.root), ...childrenOf(m.role.root).map((c) => c.id)]
                  .filter((x, i, a) => a.indexOf(x) === i)
                  .map((id) => (
                    <option key={id} value={id}>
                      {placeOf(id)?.label}
                    </option>
                  ))}
              </select>
            </label>
            <button type="button" className={small} onClick={() => setSharing(!sharing)}>
              Share…
            </button>
            {editable ? (
              <button type="button" className={primary} onClick={() => setAdding({ step: 0, v: blank(m.role.root) })}>
                + Add
              </button>
            ) : null}
          </div>
          {sharing ? <ShareEditor d={d} m={m} /> : null}
          <div className={css({ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(440px, 1fr))', gap: '3' })}>
            {d.items.map((id) => {
              const x = visOf(m, id)
              return x ? (
                <VisCard
                  key={id}
                  v={apply(x)}
                  m={m}
                  root={root}
                  actions={
                    <>
                      <button type="button" className={small} onClick={() => setEditing({ ...x })}>
                        Explore
                      </button>
                      {editable ? (
                        <button type="button" className={small} onClick={() => m.setDashboards((ds) => ds.map((y) => (y.id === d.id ? { ...y, items: y.items.filter((z) => z !== id) } : y)))}>
                          Remove
                        </button>
                      ) : null}
                    </>
                  }
                />
              ) : null
            })}
          </div>
        </>
      ) : null}
      {editing ? (
        <Drawer title="Explore" onClose={() => setEditing(null)}>
          <TypeBar v={editing} set={(p) => setEditing((x) => (x ? { ...x, ...p } : x))} />
          <VisCard v={editing} m={m} root={root} />
          <Explorer v={editing} set={(p) => setEditing((x) => (x ? { ...x, ...p } : x))} root={m.role.root} compact />
          {editable ? (
            <div className={row}>
              <button type="button" className={primary} onClick={() => (m.saveVis(editing, null), setEditing(null))}>
                Save changes
              </button>
              <button type="button" className={ghost} onClick={() => (m.saveVis({ ...editing, id: nextId('v'), title: `${editing.title} (copy)` }, d?.id ?? null), setEditing(null))}>
                Save as new item
              </button>
            </div>
          ) : (
            <SaveBar v={{ ...editing, id: nextId('v') }} set={(p) => setEditing((x) => (x ? { ...x, ...p } : x))} m={m} onSaved={() => setEditing(null)} />
          )}
        </Drawer>
      ) : null}
      {adding ? <Guided m={m} adding={adding} setAdding={setAdding} dash={d?.id ?? ''} /> : null}
    </div>
  )
}

function Drawer({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  return (
    <div className={css({ position: 'fixed', inset: '0', bg: 'bg.backdrop', zIndex: '20', display: 'flex', justifyContent: 'flex-end' })} onClick={onClose}>
      <div role="dialog" aria-label={title} className={css({ width: 'min(640px, 95vw)', height: '100%', overflowY: 'auto', bg: 'bg.surface', p: '4', display: 'flex', flexDirection: 'column', gap: '3' })} onClick={(e) => e.stopPropagation()}>
        <div className={css({ display: 'flex', justifyContent: 'space-between' })}>
          <span className={h2}>{title}</span>
          <button type="button" className={small} onClick={onClose}>
            Close
          </button>
        </div>
        {children}
      </div>
    </div>
  )
}

const STEPS = ['What', 'Where', 'When', 'Show as']
function Guided({ m, adding, setAdding, dash }: { m: M; adding: { step: number; v: Vis }; setAdding: (a: { step: number; v: Vis } | null) => void; dash: string }) {
  const { step, v } = adding
  const set = (p: Partial<Vis>) => setAdding({ step, v: { ...v, ...p } })
  return (
    <Drawer title="Add to Dashboard" onClose={() => setAdding(null)}>
      <ol className={row}>
        {STEPS.map((s, i) => (
          <li key={s}>
            <button type="button" className={cx(small, i === step && on)} onClick={() => setAdding({ step: i, v })}>
              {i + 1}. {s}
            </button>
          </li>
        ))}
      </ol>
      {step === 0 ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          <span className={h3}>Which indicators?</span>
          {INDICATORS.map((i) => (
            <label key={i.id} className={row}>
              <input type="checkbox" checked={v.inds.includes(i.id)} onChange={() => set({ inds: v.inds.includes(i.id) ? v.inds.filter((x) => x !== i.id) : [...v.inds, i.id] })} />
              {i.title} <span className={muted}>({i.unit})</span>
            </label>
          ))}
        </div>
      ) : step === 1 ? (
        <PlacePicker v={v} set={set} root={m.role.root} />
      ) : step === 2 ? (
        <PeriodPicker v={v} set={set} />
      ) : (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          <TypeBar v={v} set={set} />
          <div className={row}>
            Compare
            {(['ind', 'place'] as const).map((s) => (
              <button key={s} type="button" className={cx(small, v.series === s && on)} onClick={() => set({ series: s })}>
                {s === 'ind' ? 'indicators' : 'places'}
              </button>
            ))}
          </div>
          <input aria-label="Title" className={field} value={v.title} onChange={(e) => set({ title: e.target.value })} />
        </div>
      )}
      <span className={h3}>Preview</span>
      <VisCard v={v} m={m} />
      <div className={row}>
        {step > 0 ? (
          <button type="button" className={ghost} onClick={() => setAdding({ step: step - 1, v })}>
            Back
          </button>
        ) : null}
        {step < STEPS.length - 1 ? (
          <button type="button" className={primary} onClick={() => setAdding({ step: step + 1, v })}>
            Next
          </button>
        ) : (
          <button type="button" className={primary} onClick={() => (m.saveVis(v, dash), setAdding(null))}>
            Add to Dashboard
          </button>
        )}
      </div>
    </Drawer>
  )
}

// --- Variant C: place scorecard -------------------------------------------------------------------------------------

function Spark({ ind, place }: { ind: Ind; place: string }) {
  const months = PERIODS.month.slice(-6)
  const vals = months.map((p) => value(ind, place, p.months).v)
  const nums = vals.filter((x): x is number => x !== null)
  const lo = Math.min(...nums)
  const hi = Math.max(...nums)
  const pts = vals.map((x, i) => (x === null ? null : `${i * 12},${16 - (hi === lo ? 8 : ((x - lo) / (hi - lo)) * 14)}`)).filter(Boolean)
  return (
    <svg viewBox="0 0 62 18" width="62" height="18" aria-label={`last 6 months: ${vals.map((x) => show(ind, x)).join(', ')}`}>
      <polyline fill="none" stroke="#374151" strokeWidth="1.2" points={pts.join(' ')} />
    </svg>
  )
}

function Scorecard({ m }: { m: M }) {
  const [place, setPlace] = useState(m.role.root)
  const [periodType, setPeriodType] = useState<PeriodType>('month')
  const [period, setPeriod] = useState(CURRENT)
  const [picked, setPicked] = useState('malaria-pos')
  const [pinned, setPinned] = useState<string | null>(null)
  useEffect(() => setPlace(m.role.root), [m.role.root])
  const p = placeOf(place) as Place
  const kids = childrenOf(place)
  const cols = [p, ...kids]
  const months = PERIODS[periodType].find((x) => x.id === period)?.months ?? [CURRENT]
  const facs = facilitiesUnder(place)
  const reported = facs.filter((f) => value(indOf('opd'), f, months).reported).length
  const target = m.dashboards.find((d) => d.kind === 'saved')
  const pin = (indId: string) => {
    const v: Vis = { id: nextId('v'), title: `${indOf(indId).title} by area`, type: 'line', inds: [indId], places: [], children: true, periodType: 'month', from: '2025-10', to: CURRENT, series: 'place', disagg: 'none' }
    m.saveVis(v, target?.id ?? null)
    setPinned(target?.title ?? null)
  }
  const th = css({ px: '2', py: '1', fontSize: 'xs', textAlign: 'right', fontWeight: 'semibold' })
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <div className={row}>
        <nav aria-label="Place" className={row}>
          {pathTo(place)
            .filter((x) => within(m.role.root, x.id))
            .map((x, i, a) => (
              <span key={x.id} className={row}>
                <button type="button" className={cx(small, x.id === place && on)} onClick={() => setPlace(x.id)}>
                  {x.label}
                </button>
                {i < a.length - 1 ? '›' : null}
              </span>
            ))}
        </nav>
        <select
          aria-label="Period type"
          className={field}
          value={periodType}
          onChange={(e) => {
            const t = e.target.value as PeriodType
            setPeriodType(t)
            setPeriod(PERIODS[t].at(-1)?.id ?? '')
          }}
        >
          {(Object.keys(PERIODS) as PeriodType[]).map((t) => (
            <option key={t} value={t}>
              {PERIOD_TYPE_LABEL[t]}
            </option>
          ))}
        </select>
        <select aria-label="Period" className={field} value={period} onChange={(e) => setPeriod(e.target.value)}>
          {PERIODS[periodType].map((x) => (
            <option key={x.id} value={x.id}>
              {x.label}
            </option>
          ))}
        </select>
        <span className={cx(chip, reported < facs.length ? css({ color: 'danger.text', borderColor: 'danger.text' }) : undefined)}>
          {reported}/{facs.length} facilities reported
        </span>
      </div>
      <div className={css({ display: 'grid', gridTemplateColumns: '1fr 340px', gap: '3', alignItems: 'start' })}>
        <div className={cx(box, css({ p: '2', overflowX: 'auto' }))}>
          <table className={css({ width: '100%', fontSize: 'sm', borderCollapse: 'collapse' })}>
            <thead>
              <tr>
                <th className={cx(th, css({ textAlign: 'left' }))}>Indicator</th>
                {cols.map((c, i) => (
                  <th key={c.id} className={th}>
                    {i === 0 || c.level === 'facility' && kids.length === 0 ? (
                      c.label
                    ) : (
                      <button type="button" className={css({ textDecoration: 'underline', cursor: 'pointer' })} onClick={() => setPlace(c.id)} title="Drill down">
                        {c.label}
                      </button>
                    )}
                  </th>
                ))}
                <th className={th}>Last 6 months</th>
                <th className={th} />
              </tr>
            </thead>
            <tbody>
              {INDICATORS.map((ind) => (
                <tr key={ind.id} className={cx(ind.id === picked && css({ outline: '2px solid', outlineColor: 'accent.default' }))} onClick={() => setPicked(ind.id)}>
                  <td className={css({ px: '2', py: '1', borderTop: '1px solid', borderColor: 'border', cursor: 'pointer' })} title={ind.unit}>
                    {ind.title}
                  </td>
                  {cols.map((c) => {
                    const val = value(ind, c.id, months)
                    return val.why ? (
                      <td key={c.id} className={css({ px: '2', textAlign: 'right', color: 'fg.muted', borderTop: '1px solid', borderColor: 'border' })} title={val.why}>
                        n/a
                      </td>
                    ) : (
                      <Cell key={c.id} ind={ind} v={val.v} />
                    )
                  })}
                  <td className={css({ px: '2', borderTop: '1px solid', borderColor: 'border', textAlign: 'right' })}>{ind.calc === 'median' && p.level !== 'facility' ? null : <Spark ind={ind} place={place} />}</td>
                  <td className={css({ px: '1', borderTop: '1px solid', borderColor: 'border' })}>
                    {m.role.author ? (
                      <button type="button" className={small} onClick={(e) => (e.stopPropagation(), pin(ind.id))}>
                        Pin
                      </button>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className={muted}>n/a: a median shows at facility level only. — : not synced, never counted as zero. Click an area's name to drill down; click a row to map it.</div>
          {pinned ? <div className={css({ fontSize: 'sm', color: 'success.text' })}>Pinned to {pinned}.</div> : null}
        </div>
        <div className={cx(box, css({ p: '2', display: 'flex', flexDirection: 'column', gap: '1' }))}>
          <span className={h3}>
            {indOf(picked).title}, {PERIODS[periodType].find((x) => x.id === period)?.label}
          </span>
          <MapView v={{ ...(INITIAL_VIS[3] as Vis), inds: [picked], periodType, from: period, to: period }} places={kids.length ? kids : [p]} onPick={(id) => (childrenOf(id).length ? setPlace(id) : null)} />
        </div>
      </div>
      {target ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          <span className={h2}>{target.title}</span>
          <div className={css({ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(420px, 1fr))', gap: '3' })}>
            {target.items.map((id) => {
              const x = visOf(m, id)
              return x ? <VisCard key={id} v={x} m={m} root={place} /> : null
            })}
          </div>
        </div>
      ) : null}
    </div>
  )
}

// --- Shell ---------------------------------------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'explorer first', View: ExplorerFirst },
  { key: 'B', name: 'dashboard first', View: DashboardFirst },
  { key: 'C', name: 'place scorecard', View: Scorecard },
]

export function DashboardsPrototype() {
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
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '3', pb: '16' })}>
      <div className={cx(box, css({ p: '2', display: 'flex', gap: '3', alignItems: 'center', flexWrap: 'wrap' }))}>
        <label className={row}>
          Signed in as
          <select aria-label="Role" className={field} value={m.role.id} onChange={(e) => m.setRole(e.target.value)}>
            {ROLES.map((r) => (
              <option key={r.id} value={r.id}>
                {r.label}
              </option>
            ))}
          </select>
        </label>
        <span className={chip}>Area: {placeOf(m.role.root)?.label}</span>
        <span className={chip}>{m.role.node === 'edge' ? 'Facility Edge Node: works offline, this facility only' : 'Central'}</span>
        <span className={chip}>{m.role.author ? 'analytics.author' : 'view only'}</span>
        <span className={muted}>Summaries only (ADR 0075); today 2 Apr 2026</span>
      </div>
      <View key={`${current.key}-${m.role.id}`} m={m} />
      {import.meta.env.PROD ? null : (
        <div role="toolbar" aria-label="Prototype variants" className={css({ position: 'fixed', bottom: '4', left: '50%', transform: 'translateX(-50%)', display: 'flex', gap: '3', alignItems: 'center', bg: 'fg.default', color: 'bg.canvas', px: '4', py: '2', borderRadius: 'full', boxShadow: 'lg', zIndex: '30' })}>
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
