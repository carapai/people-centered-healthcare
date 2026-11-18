// PROTOTYPE (#647), throwaway: three visual directions on /prototype/visual-direction, switched with
// ?variant=A|B|C (← / → keys or the floating bar), each in ?theme=light|dark|contrast and
// ?density=comfortable|compact. Made-up patients; nothing is saved. Colours are Radix Colors steps
// (ADR 0080); solid accents use step 11 in light mode so white text meets WCAG AA. Fonts are system
// stacks only (no CDN, offline edges): the real font is still fog on the map.
// A: Calm clinical. Slate + teal, 6 px radius, left sidebar, cards on a tinted canvas, pages.
// B: Dense workstation. Sand + indigo, 2 px radius, top menu bar, master-detail (worklist always
//    visible beside the chart), no cards, hairline rules, the form opens as a side sheet.
// C: Friendly guided. Olive + grass, 14 px radius, rounded system font, pill navigation, a
//    "call next" hero over the queue, a patient banner, and the form as a stepped wizard.
import { useNavigate, useSearch } from '@tanstack/react-router'
import { useEffect, useRef, useState, type ReactNode } from 'react'

// --- Made-up data -----------------------------------------------------------------------------------------

type Triage = 'red' | 'yellow' | 'green'
type Pt = { id: string; name: string; sex: 'F' | 'M'; age: string; queue: string; wait: number; triage: Triage; status: string }
const PATIENTS: Pt[] = [
  { id: 'CF-0041-2291', name: 'Nakato Sarah', sex: 'F', age: '28 y', queue: 'Antenatal', wait: 12, triage: 'yellow', status: 'Vitals taken' },
  { id: 'CM-0187-5512', name: 'Okello Joseph', sex: 'M', age: '4 y', queue: 'OPD', wait: 3, triage: 'red', status: 'Waiting' },
  { id: 'CF-0099-1820', name: 'Akello Grace', sex: 'F', age: '61 y', queue: 'OPD', wait: 41, triage: 'green', status: 'Waiting' },
  { id: 'CM-0312-7740', name: 'Mugisha Ivan', sex: 'M', age: '35 y', queue: 'ART clinic', wait: 27, triage: 'green', status: 'In consultation' },
  { id: 'CF-0024-0094', name: 'Namubiru Esther', sex: 'F', age: '9 mo', queue: 'Immunisation', wait: 8, triage: 'green', status: 'Waiting' },
  { id: 'CM-0450-3318', name: 'Ssempala David', sex: 'M', age: '52 y', queue: 'OPD', wait: 19, triage: 'yellow', status: 'Lab pending' },
  { id: 'CF-0271-6603', name: 'Atim Florence', sex: 'F', age: '23 y', queue: 'Antenatal', wait: 33, triage: 'green', status: 'Waiting' },
]
const CHART = {
  allergies: ['Penicillin: rash (2023)'],
  problems: ['Pregnancy, 24 weeks (G2P1)', 'Anaemia in pregnancy'],
  meds: ['Ferrous sulphate 200 mg, once daily', 'Folic acid 5 mg, once daily'],
  vitals: [
    { k: 'BP', v: '118/76', u: 'mmHg', tone: 'ok' },
    { k: 'Temp', v: '36.8', u: '°C', tone: 'ok' },
    { k: 'Weight', v: '64.2', u: 'kg', tone: 'ok' },
    { k: 'Hb', v: '9.8', u: 'g/dL', tone: 'warn' },
  ],
  timeline: [
    { when: 'Today 09:12', what: 'Vitals recorded', who: 'Triage nurse' },
    { when: '14 Oct 2026', what: 'ANC visit 2: haemoglobin 9.8 g/dL, iron started', who: 'Midwife' },
    { when: '16 Sep 2026', what: 'ANC visit 1: booked, HIV test negative', who: 'Midwife' },
    { when: '02 Mar 2025', what: 'OPD: malaria (RDT positive), treated', who: 'Clinical officer' },
  ],
}

// --- Tokens per direction and theme ------------------------------------------------------------------------

type Theme = 'light' | 'dark' | 'contrast'
type Density = 'comfortable' | 'compact'
type Key = 'A' | 'B' | 'C'
const VARIANTS: { key: Key; name: string }[] = [
  { key: 'A', name: 'Calm clinical' },
  { key: 'B', name: 'Dense workstation' },
  { key: 'C', name: 'Friendly guided' },
]

type T = Record<string, string>
const STATUS_LIGHT: T = { danger: '#ce2c31', 'danger-soft': '#feebec', warn: '#ab6400', 'warn-soft': '#fff7c2', ok: '#218358', 'ok-soft': '#e6f6eb' }
const STATUS_DARK: T = { danger: '#ff9592', 'danger-soft': '#3b1219', warn: '#ffca16', 'warn-soft': '#302008', ok: '#3dd68c', 'ok-soft': '#132d21' }
const CONTRAST: T = {
  canvas: '#000', surface: '#000', 'surface-2': '#000', border: '#fff', 'border-strong': '#fff', fg: '#fff', muted: '#fff',
  accent: '#ffd60a', 'accent-fg': '#000', 'accent-soft': '#000', 'accent-text': '#ffd60a', 'nav-bg': '#000', 'nav-fg': '#fff',
  danger: '#ff8080', 'danger-soft': '#000', warn: '#ffd60a', 'warn-soft': '#000', ok: '#7dff9c', 'ok-soft': '#000', shadow: 'none', focus: '#ffd60a',
}
const DIRS: Record<Key, { shape: T; light: T; dark: T }> = {
  A: {
    shape: { radius: '6px', 'radius-lg': '10px', font: 'Inter, system-ui, -apple-system, "Segoe UI", sans-serif', base: '14px' },
    light: { canvas: '#f9f9fb', surface: '#fff', 'surface-2': '#f0f0f3', border: '#e0e1e6', 'border-strong': '#cdced6', fg: '#1c2024', muted: '#60646c', accent: '#008573', 'accent-fg': '#fff', 'accent-soft': '#e0f8f3', 'accent-text': '#008573', 'nav-bg': '#fcfcfd', 'nav-fg': '#1c2024', shadow: '0 1px 2px rgb(0 0 0 / 0.06)', focus: '#12a594', ...STATUS_LIGHT },
    dark: { canvas: '#111113', surface: '#18191b', 'surface-2': '#212225', border: '#2e3135', 'border-strong': '#43484e', fg: '#edeef0', muted: '#b0b4ba', accent: '#0bd8b6', 'accent-fg': '#0d1a18', 'accent-soft': '#0d2d2a', 'accent-text': '#0bd8b6', 'nav-bg': '#18191b', 'nav-fg': '#edeef0', shadow: 'none', focus: '#0bd8b6', ...STATUS_DARK },
  },
  B: {
    shape: { radius: '2px', 'radius-lg': '3px', font: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif', base: '13px' },
    light: { canvas: '#fdfdfc', surface: '#fdfdfc', 'surface-2': '#f1f0ef', border: '#e2e1de', 'border-strong': '#cfceca', fg: '#21201c', muted: '#63635e', accent: '#3e63dd', 'accent-fg': '#fff', 'accent-soft': '#edf2fe', 'accent-text': '#3a5bc7', 'nav-bg': '#21201c', 'nav-fg': '#eeeeec', shadow: 'none', focus: '#3e63dd', ...STATUS_LIGHT },
    dark: { canvas: '#111110', surface: '#191918', 'surface-2': '#222221', border: '#31312e', 'border-strong': '#494844', fg: '#eeeeec', muted: '#b5b3ad', accent: '#3e63dd', 'accent-fg': '#fff', 'accent-soft': '#182449', 'accent-text': '#9eb1ff', 'nav-bg': '#000', 'nav-fg': '#eeeeec', shadow: 'none', focus: '#9eb1ff', ...STATUS_DARK },
  },
  C: {
    shape: { radius: '14px', 'radius-lg': '22px', font: 'ui-rounded, "SF Pro Rounded", "Nunito", system-ui, sans-serif', base: '15px' },
    light: { canvas: '#eff1ef', surface: '#fff', 'surface-2': '#f8faf8', border: '#e7e9e7', 'border-strong': '#cccfcc', fg: '#1d211c', muted: '#606650', accent: '#2a7e3b', 'accent-fg': '#fff', 'accent-soft': '#e9f6e9', 'accent-text': '#2a7e3b', 'nav-bg': '#fff', 'nav-fg': '#1d211c', shadow: '0 4px 16px rgb(29 33 28 / 0.08)', focus: '#46a758', ...STATUS_LIGHT },
    dark: { canvas: '#111210', surface: '#181917', 'surface-2': '#212220', border: '#2a2b28', 'border-strong': '#3c3f3a', fg: '#eceeec', muted: '#afb5ad', accent: '#71d083', 'accent-fg': '#10170f', 'accent-soft': '#1b2a1e', 'accent-text': '#71d083', 'nav-bg': '#181917', 'nav-fg': '#eceeec', shadow: 'none', focus: '#71d083', ...STATUS_DARK },
  },
}
const vars = (t: T) => Object.entries(t).map(([k, v]) => `--${k}:${v};`).join('')
const TOKENS = (Object.keys(DIRS) as Key[])
  .map((k) => {
    const d = DIRS[k]
    const sel = `.pv[data-dir="${k}"]`
    return `${sel}{${vars(d.shape)}}${sel}[data-theme="light"]{${vars(d.light)}}${sel}[data-theme="dark"]{${vars(d.dark)}}${sel}[data-theme="contrast"]{${vars(CONTRAST)}}`
  })
  .join('\n')

const CSS = `
${TOKENS}
.pv{position:fixed;inset:0;z-index:50;overflow:auto;background:var(--canvas);color:var(--fg);font-family:var(--font);font-size:var(--base);line-height:1.45;--ctrl:44px;--pad:16px;--gap:16px;--row:44px}
.pv[data-density="compact"]{--ctrl:32px;--pad:10px;--gap:10px;--row:32px}
.pv *{box-sizing:border-box}
.pv :focus-visible{outline:2px solid var(--focus);outline-offset:2px}
.pv h1,.pv h2,.pv h3{margin:0;line-height:1.25}
.pv p{margin:0}
.pv-muted{color:var(--muted)}
.pv-btn{min-height:var(--ctrl);padding:0 calc(var(--pad) + 2px);border-radius:var(--radius);border:1px solid var(--border-strong);background:var(--surface);color:var(--fg);font:inherit;font-weight:500;cursor:pointer;display:inline-flex;align-items:center;gap:6px}
.pv-btn:hover{background:var(--surface-2)}
.pv-solid{background:var(--accent);color:var(--accent-fg);border-color:var(--accent)}
.pv-solid:hover{background:var(--accent);filter:brightness(0.92)}
.pv-danger{color:var(--danger);border-color:var(--danger)}
.pv-input{min-height:var(--ctrl);width:100%;padding:0 var(--pad);border-radius:var(--radius);border:1px solid var(--border-strong);background:var(--surface);color:var(--fg);font:inherit}
textarea.pv-input{padding:8px var(--pad);min-height:calc(var(--ctrl) * 2)}
.pv-label{display:flex;flex-direction:column;gap:4px;font-weight:500}
.pv-hint{font-weight:400;color:var(--muted);font-size:0.9em}
.pv-badge{display:inline-flex;align-items:center;gap:4px;padding:1px 8px;border-radius:999px;font-size:0.85em;font-weight:600;border:1px solid transparent;white-space:nowrap}
.pv-red{background:var(--danger-soft);color:var(--danger);border-color:var(--danger)}
.pv-yellow{background:var(--warn-soft);color:var(--warn);border-color:var(--warn)}
.pv-green{background:var(--ok-soft);color:var(--ok);border-color:var(--ok)}
.pv-table{width:100%;border-collapse:collapse}
.pv-table th{text-align:left;font-weight:600;color:var(--muted);font-size:0.85em;padding:0 var(--pad);height:var(--row);border-bottom:1px solid var(--border-strong);white-space:nowrap}
.pv-table td{padding:0 var(--pad);height:var(--row);border-bottom:1px solid var(--border)}
.pv-table tbody tr{cursor:pointer}
.pv-table tbody tr:hover td{background:var(--surface-2)}
.pv-table tr[aria-selected="true"] td{background:var(--accent-soft)}
.pv-mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:0.9em}
.pv-num{font-variant-numeric:tabular-nums}
.pv dialog{border:1px solid var(--border-strong);border-radius:var(--radius-lg);background:var(--surface);color:var(--fg);padding:0;width:min(480px,92vw);box-shadow:0 16px 48px rgb(0 0 0 / 0.25)}
.pv dialog::backdrop{background:rgb(0 0 0 / 0.45)}
.pv-dlg{padding:calc(var(--pad) * 1.5);display:flex;flex-direction:column;gap:var(--gap)}
.pv-row{display:flex;gap:var(--gap);align-items:center;flex-wrap:wrap}
.pv-stack{display:flex;flex-direction:column;gap:var(--gap)}
.pv-navlink{display:flex;align-items:center;gap:10px;width:100%;min-height:var(--ctrl);padding:0 12px;border:0;background:transparent;color:inherit;font:inherit;text-align:left;cursor:pointer;border-radius:var(--radius)}
.pv-sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}

/* A: Calm clinical */
.pvA-shell{display:grid;grid-template-columns:232px 1fr;min-height:100%}
.pvA-side{background:var(--nav-bg);border-right:1px solid var(--border);padding:16px 10px;display:flex;flex-direction:column;gap:2px}
.pvA-side .pv-navlink:hover{background:var(--surface-2)}
.pvA-side .pv-navlink[aria-current="page"]{background:var(--accent-soft);color:var(--accent-text);font-weight:600}
.pvA-group{font-size:0.75em;text-transform:uppercase;letter-spacing:0.06em;color:var(--muted);padding:14px 12px 4px}
.pvA-head{height:60px;display:flex;align-items:center;gap:16px;padding:0 24px;border-bottom:1px solid var(--border);background:var(--surface)}
.pvA-main{padding:24px;display:flex;flex-direction:column;gap:var(--gap);max-width:1200px}
.pvA-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow)}
.pvA-cardh{padding:var(--pad) calc(var(--pad) + 4px);border-bottom:1px solid var(--border);display:flex;align-items:center;justify-content:space-between;gap:8px}
.pvA-cardb{padding:var(--pad) calc(var(--pad) + 4px)}
.pvA-grid{display:grid;grid-template-columns:320px 1fr;gap:var(--gap);align-items:start}
.pvA-vitals{display:grid;grid-template-columns:repeat(4,1fr);gap:var(--gap)}
.pvA-tabs{display:flex;gap:4px;border-bottom:1px solid var(--border)}
.pvA-tab{border:0;background:none;font:inherit;color:var(--muted);padding:10px 14px;cursor:pointer;border-bottom:2px solid transparent;margin-bottom:-1px}
.pvA-tab[aria-selected="true"]{color:var(--accent-text);border-bottom-color:var(--accent);font-weight:600}

/* B: Dense workstation */
.pvB-shell{display:grid;grid-template-rows:auto 1fr auto;height:100%}
.pvB-bar{background:var(--nav-bg);color:var(--nav-fg);display:flex;align-items:stretch;gap:0;padding:0 12px;height:40px}
.pvB-brand{font-weight:700;letter-spacing:0.02em;display:flex;align-items:center;padding-right:18px;margin-right:6px;border-right:1px solid rgb(255 255 255 / 0.15)}
.pvB-menu{border:0;background:none;color:inherit;font:inherit;padding:0 12px;cursor:pointer;opacity:0.8;border-bottom:2px solid transparent}
.pvB-menu[aria-current="page"]{opacity:1;border-bottom-color:var(--accent);font-weight:600}
.pv[data-theme="contrast"] .pvB-bar{border-bottom:1px solid #fff}
.pvB-split{display:grid;grid-template-columns:480px 1fr;min-height:0}
.pv[data-dir="B"] .pvB-list td{white-space:nowrap}
.pvB-list{border-right:1px solid var(--border-strong);overflow:auto;background:var(--surface)}
.pvB-pane{overflow:auto;min-width:0}
.pvB-toolbar{display:flex;gap:8px;align-items:center;padding:6px 10px;border-bottom:1px solid var(--border);background:var(--surface-2)}
.pvB-toolbar .pv-input,.pvB-toolbar .pv-btn{min-height:calc(var(--ctrl) - 4px)}
.pv[data-dir="B"] .pv-table th{text-transform:uppercase;letter-spacing:0.04em;font-size:0.75em;background:var(--surface-2)}
.pv[data-dir="B"] .pv-table tbody tr:nth-child(even) td{background:var(--surface-2)}
.pv[data-dir="B"] .pv-table tr[aria-selected="true"] td{background:var(--accent-soft);box-shadow:inset 3px 0 0 var(--accent)}
.pv[data-dir="B"] .pv-badge{border-radius:2px;padding:0 5px;font-size:0.78em;text-transform:uppercase;letter-spacing:0.03em}
.pvB-pt{display:flex;gap:24px;align-items:baseline;padding:10px 16px;border-bottom:1px solid var(--border-strong);flex-wrap:wrap}
.pvB-tabs{display:flex;border-bottom:1px solid var(--border-strong);padding:0 8px;background:var(--surface-2)}
.pvB-tab{border:0;background:none;font:inherit;color:var(--muted);padding:7px 12px;cursor:pointer;border-bottom:2px solid transparent}
.pvB-tab[aria-selected="true"]{color:var(--fg);border-bottom-color:var(--accent);font-weight:600;background:var(--surface)}
.pvB-cols{display:grid;grid-template-columns:1fr 1fr 1fr;border-bottom:1px solid var(--border)}
.pvB-col{padding:10px 16px;border-right:1px solid var(--border)}
.pvB-col:last-child{border-right:0}
.pvB-h{font-size:0.75em;text-transform:uppercase;letter-spacing:0.05em;color:var(--muted);font-weight:600;margin-bottom:6px}
.pvB-kv{display:grid;grid-template-columns:repeat(4,auto);justify-content:start;gap:2px 28px;padding:10px 16px;border-bottom:1px solid var(--border)}
.pvB-status{display:flex;gap:18px;padding:3px 12px;font-size:0.8em;background:var(--surface-2);border-top:1px solid var(--border-strong);color:var(--muted)}
.pvB-sheet{position:fixed;top:40px;right:0;bottom:0;width:min(440px,100vw);background:var(--surface);border-left:1px solid var(--border-strong);box-shadow:-12px 0 32px rgb(0 0 0 / 0.18);display:flex;flex-direction:column;z-index:2}
.pvB-form{display:grid;grid-template-columns:130px 1fr;gap:8px 12px;align-items:center;padding:14px 16px}
.pvB-form label{font-weight:500;text-align:right}

/* C: Friendly guided */
.pvC-head{display:flex;align-items:center;gap:18px;padding:14px 28px;background:var(--nav-bg);box-shadow:var(--shadow);border-bottom:1px solid var(--border)}
.pvC-logo{width:36px;height:36px;border-radius:12px;background:var(--accent);color:var(--accent-fg);display:grid;place-items:center;font-weight:800}
.pvC-pills{display:flex;gap:6px;background:var(--surface-2);padding:4px;border-radius:999px;border:1px solid var(--border)}
.pvC-pill{border:0;background:none;font:inherit;color:var(--fg);padding:0 18px;min-height:calc(var(--ctrl) - 8px);border-radius:999px;cursor:pointer;font-weight:500}
.pvC-pill[aria-current="page"]{background:var(--accent);color:var(--accent-fg)}
.pv[data-dir="C"] .pv-btn{border-radius:999px}
.pv[data-dir="C"] .pv-input{border-radius:var(--radius);border-width:1.5px}
.pvC-main{max-width:1080px;margin:0 auto;padding:28px;display:flex;flex-direction:column;gap:calc(var(--gap) + 6px)}
.pvC-card{background:var(--surface);border-radius:var(--radius-lg);box-shadow:var(--shadow);border:1px solid var(--border);padding:calc(var(--pad) + 8px)}
.pvC-hero{display:flex;align-items:center;justify-content:space-between;gap:20px;background:var(--accent-soft);border:1px solid var(--border);border-radius:var(--radius-lg);padding:calc(var(--pad) + 10px)}
.pvC-big{font-size:1.9em;font-weight:700;letter-spacing:-0.01em}
.pvC-avatar{width:64px;height:64px;border-radius:50%;background:var(--accent-soft);color:var(--accent-text);display:grid;place-items:center;font-weight:800;font-size:1.4em;border:2px solid var(--accent)}
.pvC-grid3{display:grid;grid-template-columns:repeat(3,1fr);gap:var(--gap)}
.pvC-vital{background:var(--surface-2);border-radius:var(--radius);padding:var(--pad);border:1px solid var(--border)}
.pvC-steps{display:flex;gap:8px}
.pvC-step{flex:1;display:flex;align-items:center;gap:8px;padding:10px 14px;border-radius:999px;background:var(--surface-2);color:var(--muted);border:1px solid var(--border)}
.pvC-step[aria-current="step"]{background:var(--accent-soft);color:var(--accent-text);font-weight:600;border-color:var(--accent)}
.pvC-dot{width:24px;height:24px;border-radius:50%;display:grid;place-items:center;background:var(--surface);font-size:0.85em;font-weight:700;border:1px solid currentColor}
.pv[data-dir="C"] .pv-table th{border-bottom-color:var(--border)}
.pv[data-dir="C"] .pv-table td{border-bottom:0}
.pv[data-dir="C"] .pv-table tbody tr:nth-child(odd) td{background:var(--surface-2)}
.pv[data-dir="C"] .pv-table tbody tr td:first-child{border-radius:var(--radius) 0 0 var(--radius)}
.pv[data-dir="C"] .pv-table tbody tr td:last-child{border-radius:0 var(--radius) var(--radius) 0}

/* the prototype's own switcher: deliberately not part of any direction */
.pvbar{position:fixed;left:50%;bottom:16px;transform:translateX(-50%);z-index:60;display:flex;align-items:center;gap:8px;padding:6px 10px;border-radius:999px;background:#111;color:#fff;font:13px/1.2 system-ui,sans-serif;box-shadow:0 8px 24px rgb(0 0 0 / 0.35);border:1px solid #555}
.pvbar button{background:#2a2a2a;color:#fff;border:1px solid #555;border-radius:999px;padding:5px 11px;cursor:pointer;font:inherit}
.pvbar button[aria-pressed="true"]{background:#fff;color:#111}
.pvbar .lbl{min-width:170px;text-align:center;font-weight:600}
.pvbar .sep{width:1px;height:20px;background:#555}
`

// --- Shared bits (look comes from the tokens; layout is each variant's own) -------------------------------

const TRIAGE_LABEL: Record<Triage, string> = { red: 'Emergency', yellow: 'Priority', green: 'Queue' }
const Triage = ({ t }: { t: Triage }) => (
  <span className={`pv-badge pv-${t}`}>
    <span aria-hidden>{t === 'red' ? '▲' : t === 'yellow' ? '◆' : '●'}</span>
    {TRIAGE_LABEL[t]}
  </span>
)

function Worklist({ selected, onSelect, cols = 'full' }: { selected: string; onSelect: (id: string) => void; cols?: 'full' | 'narrow' }) {
  return (
    <table className="pv-table" aria-label="Today's worklist">
      <thead>
        <tr>
          <th scope="col">Patient</th>
          {cols === 'full' ? <th scope="col">Health ID</th> : null}
          <th scope="col">Queue</th>
          <th scope="col" className="pv-num">Waiting</th>
          <th scope="col">Triage</th>
          {cols === 'full' ? <th scope="col">Status</th> : null}
        </tr>
      </thead>
      <tbody>
        {PATIENTS.map((p) => (
          <tr key={p.id} aria-selected={p.id === selected} onClick={() => onSelect(p.id)} tabIndex={0} onKeyDown={(e) => e.key === 'Enter' && onSelect(p.id)}>
            <td>
              <strong>{p.name}</strong> <span className="pv-muted">{p.sex}, {p.age}</span>
            </td>
            {cols === 'full' ? <td className="pv-mono">{p.id}</td> : null}
            <td>{p.queue}</td>
            <td className="pv-num">{p.wait} min</td>
            <td><Triage t={p.triage} /></td>
            {cols === 'full' ? <td>{p.status}</td> : null}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function useDialog() {
  const ref = useRef<HTMLDialogElement>(null)
  return { ref, open: () => ref.current?.showModal(), close: () => ref.current?.close() }
}

function EndVisitDialog({ d, patient }: { d: ReturnType<typeof useDialog>; patient: Pt }) {
  return (
    <dialog ref={d.ref} aria-labelledby="pv-dlg-title">
      <form method="dialog" className="pv-dlg">
        <h2 id="pv-dlg-title">End visit without consultation?</h2>
        <p className="pv-muted">{patient.name} leaves the Antenatal queue. Vitals stay on the record; the visit is closed as "left before seen".</p>
        <label className="pv-label">
          Reason
          <textarea className="pv-input" defaultValue="Patient left before consultation" />
        </label>
        <div className="pv-row" style={{ justifyContent: 'flex-end' }}>
          <button className="pv-btn" value="cancel">Keep in queue</button>
          <button className="pv-btn pv-solid" value="end">End visit</button>
        </div>
      </form>
    </dialog>
  )
}

function VitalsFields({ layout }: { layout: 'stack' | 'grid' }) {
  const f = [
    { l: 'Systolic BP', u: 'mmHg', v: '118' },
    { l: 'Diastolic BP', u: 'mmHg', v: '76' },
    { l: 'Temperature', u: '°C', v: '36.8' },
    { l: 'Weight', u: 'kg', v: '64.2' },
  ]
  if (layout === 'grid')
    return (
      <>
        {f.map((x) => (
          <FieldPair key={x.l} label={`${x.l} (${x.u})`} value={x.v} />
        ))}
      </>
    )
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 'var(--gap)' }}>
      {f.map((x) => (
        <label key={x.l} className="pv-label">
          {x.l} <span className="pv-hint">{x.u}</span>
          <input className="pv-input pv-num" defaultValue={x.v} inputMode="decimal" />
        </label>
      ))}
    </div>
  )
}
function FieldPair({ label, value }: { label: string; value: string }) {
  const id = `pvf-${label.replace(/\W+/g, '-')}`
  return (
    <>
      <label htmlFor={id}>{label}</label>
      <input id={id} className="pv-input pv-num" defaultValue={value} />
    </>
  )
}

type Page = 'worklist' | 'chart' | 'form'
type VP = { patient: Pt; setPatient: (id: string) => void }

// --- A: Calm clinical --------------------------------------------------------------------------------------

function VariantA({ patient, setPatient }: VP) {
  const [page, setPage] = useState<Page>('worklist')
  const [tab, setTab] = useState('summary')
  const dlg = useDialog()
  const nav = (p: Page, label: string, icon: string) => (
    <button className="pv-navlink" aria-current={page === p ? 'page' : undefined} onClick={() => setPage(p)}>
      <span aria-hidden>{icon}</span>
      {label}
    </button>
  )
  return (
    <div className="pvA-shell">
      <nav className="pvA-side" aria-label="Main">
        <div className="pv-row" style={{ padding: '4px 12px 12px', gap: 10 }}>
          <span aria-hidden style={{ width: 28, height: 28, borderRadius: 8, background: 'var(--accent)', color: 'var(--accent-fg)', display: 'grid', placeItems: 'center', fontWeight: 700 }}>+</span>
          <div>
            <strong>Kawempe HC IV</strong>
            <div className="pv-muted" style={{ fontSize: '0.85em' }}>Edge · synced 2 min ago</div>
          </div>
        </div>
        <div className="pvA-group">Clinical</div>
        {nav('worklist', 'Worklist', '☰')}
        {nav('chart', 'Patient chart', '◉')}
        {nav('form', 'Record vitals', '✎')}
        <div className="pvA-group">Facility</div>
        <button className="pv-navlink">⧉ Stock</button>
        <button className="pv-navlink">▤ Reports</button>
        <div style={{ flex: 1 }} />
        <button className="pv-navlink">⚙ Settings</button>
      </nav>
      <div>
        <header className="pvA-head">
          <input className="pv-input" style={{ maxWidth: 420 }} placeholder="Search patients by name, phone or Health ID" aria-label="Search patients" />
          <div style={{ flex: 1 }} />
          <span className="pv-muted">Triage nurse</span>
          <span aria-label="Signed in as Apio R." style={{ width: 32, height: 32, borderRadius: '50%', background: 'var(--surface-2)', border: '1px solid var(--border)', display: 'grid', placeItems: 'center', fontWeight: 600 }}>AR</span>
        </header>
        <main className="pvA-main">
          {page === 'worklist' ? (
            <>
              <div className="pv-row" style={{ justifyContent: 'space-between' }}>
                <div>
                  <h1 style={{ fontSize: '1.5em' }}>Worklist</h1>
                  <p className="pv-muted">7 patients waiting · longest 41 min</p>
                </div>
                <button className="pv-btn pv-solid">+ Register patient</button>
              </div>
              <section className="pvA-card">
                <div className="pvA-cardh">
                  <div className="pv-row" style={{ gap: 8 }}>
                    <button className="pv-btn" aria-pressed="true">All queues</button>
                    <button className="pv-btn">Antenatal</button>
                    <button className="pv-btn">OPD</button>
                  </div>
                  <input className="pv-input" style={{ maxWidth: 220 }} placeholder="Filter" aria-label="Filter worklist" />
                </div>
                <Worklist selected={patient.id} onSelect={(id) => (setPatient(id), setPage('chart'))} />
              </section>
            </>
          ) : null}
          {page === 'chart' ? (
            <>
              <section className="pvA-card">
                <div className="pvA-cardb pv-row" style={{ justifyContent: 'space-between' }}>
                  <div>
                    <h1 style={{ fontSize: '1.4em' }}>{patient.name}</h1>
                    <p className="pv-muted">{patient.sex}, {patient.age} · <span className="pv-mono">{patient.id}</span> · {patient.queue}</p>
                  </div>
                  <div className="pv-row" style={{ gap: 8 }}>
                    <Triage t={patient.triage} />
                    <span className="pv-badge pv-red">Allergy: Penicillin</span>
                    <button className="pv-btn pv-danger" onClick={dlg.open}>End visit</button>
                    <button className="pv-btn pv-solid" onClick={() => setPage('form')}>Record vitals</button>
                  </div>
                </div>
                <div className="pvA-tabs" role="tablist" style={{ padding: '0 16px' }}>
                  {['summary', 'encounters', 'results', 'medications'].map((t) => (
                    <button key={t} role="tab" className="pvA-tab" aria-selected={tab === t} onClick={() => setTab(t)}>{t[0]?.toUpperCase() + t.slice(1)}</button>
                  ))}
                </div>
              </section>
              <div className="pvA-vitals">
                {CHART.vitals.map((v) => (
                  <div key={v.k} className="pvA-card pvA-cardb">
                    <div className="pv-muted">{v.k}</div>
                    <div className="pv-num" style={{ fontSize: '1.6em', fontWeight: 600, color: v.tone === 'warn' ? 'var(--warn)' : undefined }}>
                      {v.v} <span className="pv-muted" style={{ fontSize: '0.55em' }}>{v.u}</span>
                    </div>
                  </div>
                ))}
              </div>
              <div className="pvA-grid">
                <div className="pv-stack">
                  {[['Allergies', CHART.allergies], ['Problems', CHART.problems], ['Medications', CHART.meds]].map(([h, items]) => (
                    <section key={h as string} className="pvA-card">
                      <div className="pvA-cardh"><h2 style={{ fontSize: '1em' }}>{h as string}</h2></div>
                      <ul className="pvA-cardb" style={{ margin: 0, paddingLeft: 36 }}>
                        {(items as string[]).map((i) => <li key={i}>{i}</li>)}
                      </ul>
                    </section>
                  ))}
                </div>
                <section className="pvA-card">
                  <div className="pvA-cardh"><h2 style={{ fontSize: '1em' }}>Timeline</h2></div>
                  <ol className="pvA-cardb" style={{ listStyle: 'none', margin: 0, display: 'flex', flexDirection: 'column', gap: 'var(--gap)' }}>
                    {CHART.timeline.map((e) => (
                      <li key={e.when} style={{ display: 'grid', gridTemplateColumns: '120px 1fr', gap: 12, borderLeft: '2px solid var(--accent)', paddingLeft: 12 }}>
                        <span className="pv-muted">{e.when}</span>
                        <span>{e.what}<br /><span className="pv-muted">{e.who}</span></span>
                      </li>
                    ))}
                  </ol>
                </section>
              </div>
            </>
          ) : null}
          {page === 'form' ? (
            <section className="pvA-card" style={{ maxWidth: 640 }}>
              <div className="pvA-cardh">
                <div>
                  <h1 style={{ fontSize: '1.2em' }}>Record vitals</h1>
                  <p className="pv-muted">{patient.name} · {patient.queue}</p>
                </div>
              </div>
              <div className="pvA-cardb pv-stack">
                <VitalsFields layout="stack" />
                <label className="pv-label">Triage category
                  <select className="pv-input" defaultValue="yellow"><option value="red">Emergency</option><option value="yellow">Priority</option><option value="green">Queue</option></select>
                </label>
                <label className="pv-label">Notes <span className="pv-hint">Optional</span><textarea className="pv-input" /></label>
              </div>
              <div className="pvA-cardb pv-row" style={{ justifyContent: 'flex-end', borderTop: '1px solid var(--border)' }}>
                <button className="pv-btn" onClick={() => setPage('chart')}>Cancel</button>
                <button className="pv-btn pv-solid" onClick={() => setPage('chart')}>Save vitals</button>
              </div>
            </section>
          ) : null}
        </main>
      </div>
      <EndVisitDialog d={dlg} patient={patient} />
    </div>
  )
}

// --- B: Dense workstation ----------------------------------------------------------------------------------

function VariantB({ patient, setPatient }: VP) {
  const [tab, setTab] = useState('Overview')
  const [sheet, setSheet] = useState(false)
  const dlg = useDialog()
  return (
    <div className="pvB-shell">
      <nav className="pvB-bar" aria-label="Main">
        <span className="pvB-brand">KAWEMPE HC IV</span>
        {['Clinical', 'Laboratory', 'Pharmacy', 'Stock', 'Reports'].map((m, i) => (
          <button key={m} className="pvB-menu" aria-current={i === 0 ? 'page' : undefined}>{m}</button>
        ))}
        <div style={{ flex: 1 }} />
        <span style={{ display: 'flex', alignItems: 'center', gap: 14, fontSize: '0.9em', opacity: 0.85 }}>
          <span>⌘K Search</span><span>Apio R. · Triage</span>
        </span>
      </nav>
      <div className="pvB-split">
        <section className="pvB-list" aria-label="Worklist">
          <div className="pvB-toolbar">
            <input className="pv-input" placeholder="Filter 7 patients" aria-label="Filter worklist" />
            <select className="pv-input" style={{ width: 130 }} aria-label="Queue"><option>All queues</option><option>Antenatal</option></select>
          </div>
          <Worklist selected={patient.id} onSelect={setPatient} cols="narrow" />
        </section>
        <section className="pvB-pane" aria-label="Patient chart">
          <div className="pvB-pt">
            <h1 style={{ fontSize: '1.25em' }}>{patient.name}</h1>
            <span className="pv-muted">{patient.sex} · {patient.age}</span>
            <span className="pv-mono">{patient.id}</span>
            <Triage t={patient.triage} />
            <span className="pv-badge pv-red">Penicillin</span>
            <div style={{ flex: 1 }} />
            <div className="pv-row" style={{ gap: 6 }}>
              <button className="pv-btn" onClick={() => setSheet(true)}>Vitals… <span className="pv-muted">V</span></button>
              <button className="pv-btn">Order… <span className="pv-muted">O</span></button>
              <button className="pv-btn pv-danger" onClick={dlg.open}>End visit</button>
            </div>
          </div>
          <div className="pvB-tabs" role="tablist">
            {['Overview', 'Encounters', 'Results', 'Medications', 'Documents'].map((t) => (
              <button key={t} role="tab" className="pvB-tab" aria-selected={tab === t} onClick={() => setTab(t)}>{t}</button>
            ))}
          </div>
          <div className="pvB-kv">
            {CHART.vitals.map((v) => (
              <div key={v.k}>
                <div className="pvB-h">{v.k}</div>
                <span className="pv-num" style={{ fontWeight: 600, color: v.tone === 'warn' ? 'var(--warn)' : undefined }}>{v.v}</span> <span className="pv-muted">{v.u}</span>
              </div>
            ))}
          </div>
          <div className="pvB-cols">
            {[['Allergies', CHART.allergies], ['Problems', CHART.problems], ['Medications', CHART.meds]].map(([h, items]) => (
              <div key={h as string} className="pvB-col">
                <h2 className="pvB-h">{h as string}</h2>
                {(items as string[]).map((i) => <div key={i}>{i}</div>)}
              </div>
            ))}
          </div>
          <table className="pv-table" aria-label="Encounters">
            <thead><tr><th scope="col">When</th><th scope="col">Event</th><th scope="col">By</th></tr></thead>
            <tbody>
              {CHART.timeline.map((e) => (
                <tr key={e.when}><td className="pv-num" style={{ whiteSpace: 'nowrap' }}>{e.when}</td><td>{e.what}</td><td className="pv-muted">{e.who}</td></tr>
              ))}
            </tbody>
          </table>
        </section>
      </div>
      <footer className="pvB-status"><span>● Edge online</span><span>Synced 09:14</span><span>7 waiting · longest 41 min</span><span>Build 2026.11.3</span></footer>
      {sheet ? (
        <aside className="pvB-sheet" aria-label="Record vitals">
          <div className="pvB-pt" style={{ justifyContent: 'space-between' }}>
            <h2 style={{ fontSize: '1.05em' }}>Record vitals · {patient.name}</h2>
            <button className="pv-btn" onClick={() => setSheet(false)} aria-label="Close">✕</button>
          </div>
          <div className="pvB-form">
            <VitalsFields layout="grid" />
            <label htmlFor="pvB-tri">Triage</label>
            <select id="pvB-tri" className="pv-input" defaultValue="yellow"><option value="red">Emergency</option><option value="yellow">Priority</option><option value="green">Queue</option></select>
            <label htmlFor="pvB-notes" style={{ alignSelf: 'start', paddingTop: 6 }}>Notes</label>
            <textarea id="pvB-notes" className="pv-input" />
          </div>
          <div style={{ flex: 1 }} />
          <div className="pvB-toolbar" style={{ justifyContent: 'flex-end', borderTop: '1px solid var(--border-strong)' }}>
            <button className="pv-btn" onClick={() => setSheet(false)}>Cancel <span className="pv-muted">Esc</span></button>
            <button className="pv-btn pv-solid" onClick={() => setSheet(false)}>Save <span style={{ opacity: 0.75 }}>⌘↵</span></button>
          </div>
        </aside>
      ) : null}
      <EndVisitDialog d={dlg} patient={patient} />
    </div>
  )
}

// --- C: Friendly guided ------------------------------------------------------------------------------------

function VariantC({ patient, setPatient }: VP) {
  const [page, setPage] = useState<Page>('worklist')
  const [step, setStep] = useState(1)
  const dlg = useDialog()
  const next = PATIENTS.find((p) => p.triage === 'red') ?? (PATIENTS[0] as Pt)
  const pill = (p: Page, label: string) => (
    <button className="pvC-pill" aria-current={page === p ? 'page' : undefined} onClick={() => setPage(p)}>{label}</button>
  )
  const initials = patient.name.split(' ').map((s) => s[0]).join('')
  return (
    <div>
      <header className="pvC-head">
        <span className="pvC-logo" aria-hidden>K</span>
        <strong style={{ fontSize: '1.05em' }}>Kawempe HC IV</strong>
        <nav aria-label="Main" className="pvC-pills">
          {pill('worklist', 'Today')}
          {pill('chart', 'Patient')}
          {pill('form', 'Record vitals')}
        </nav>
        <div style={{ flex: 1 }} />
        <button className="pv-btn">🔍 Find a patient</button>
        <span className="pvC-avatar" style={{ width: 40, height: 40, fontSize: '0.95em' }} aria-label="Signed in as Apio R.">AR</span>
      </header>
      <main className="pvC-main">
        {page === 'worklist' ? (
          <>
            <section className="pvC-hero" aria-label="Next patient">
              <div>
                <p className="pv-muted">Good morning, Apio. Next up</p>
                <div className="pvC-big">{next.name}</div>
                <div className="pv-row" style={{ gap: 8, marginTop: 6 }}>
                  <Triage t={next.triage} /> <span className="pv-muted">{next.sex}, {next.age} · {next.queue} · waiting {next.wait} min</span>
                </div>
              </div>
              <button className="pv-btn pv-solid" style={{ fontSize: '1.1em', minHeight: 'calc(var(--ctrl) + 8px)', padding: '0 28px' }} onClick={() => (setPatient(next.id), setPage('chart'))}>Call next patient →</button>
            </section>
            <div className="pvC-grid3">
              {[['7', 'waiting'], ['41 min', 'longest wait'], ['23', 'seen today']].map(([n, l]) => (
                <div key={l} className="pvC-card"><div className="pvC-big pv-num">{n}</div><div className="pv-muted">{l}</div></div>
              ))}
            </div>
            <section className="pvC-card pv-stack">
              <div className="pv-row" style={{ justifyContent: 'space-between' }}>
                <h1 style={{ fontSize: '1.3em' }}>Today's queue</h1>
                <button className="pv-btn">+ Register someone new</button>
              </div>
              <Worklist selected={patient.id} onSelect={(id) => (setPatient(id), setPage('chart'))} />
            </section>
          </>
        ) : null}
        {page === 'chart' ? (
          <>
            <section className="pvC-card pv-row" style={{ gap: 20 }}>
              <span className="pvC-avatar" aria-hidden>{initials}</span>
              <div style={{ flex: 1 }}>
                <h1 style={{ fontSize: '1.6em' }}>{patient.name}</h1>
                <p className="pv-muted">{patient.sex === 'F' ? 'Female' : 'Male'}, {patient.age} · {patient.queue} · <span className="pv-mono">{patient.id}</span></p>
                <div className="pv-row" style={{ gap: 8, marginTop: 8 }}>
                  <Triage t={patient.triage} /><span className="pv-badge pv-red">⚠ Allergic to penicillin</span><span className="pv-badge pv-yellow">Low haemoglobin</span>
                </div>
              </div>
              <div className="pv-stack" style={{ gap: 8 }}>
                <button className="pv-btn pv-solid" onClick={() => (setStep(1), setPage('form'))}>Record vitals</button>
                <button className="pv-btn pv-danger" onClick={dlg.open}>End visit</button>
              </div>
            </section>
            <div className="pvC-grid3" style={{ gridTemplateColumns: 'repeat(4,1fr)' }}>
              {CHART.vitals.map((v) => (
                <div key={v.k} className="pvC-vital">
                  <div className="pv-muted">{v.k}</div>
                  <div className="pv-num" style={{ fontSize: '1.7em', fontWeight: 700, color: v.tone === 'warn' ? 'var(--warn)' : undefined }}>{v.v}</div>
                  <div className="pv-muted">{v.u}</div>
                </div>
              ))}
            </div>
            <div className="pvC-grid3">
              {[['💊 Medicines', CHART.meds], ['📋 Conditions', CHART.problems], ['⚠ Allergies', CHART.allergies]].map(([h, items]) => (
                <section key={h as string} className="pvC-card">
                  <h2 style={{ fontSize: '1.05em', marginBottom: 10 }}>{h as string}</h2>
                  {(items as string[]).map((i) => <p key={i} style={{ padding: '6px 0' }}>{i}</p>)}
                </section>
              ))}
            </div>
            <section className="pvC-card">
              <h2 style={{ fontSize: '1.05em', marginBottom: 12 }}>Story so far</h2>
              {CHART.timeline.map((e) => (
                <div key={e.when} className="pv-row" style={{ padding: '8px 0', alignItems: 'flex-start' }}>
                  <span className="pv-badge pv-green" style={{ minWidth: 110, justifyContent: 'center' }}>{e.when}</span>
                  <span>{e.what} <span className="pv-muted">· {e.who}</span></span>
                </div>
              ))}
            </section>
          </>
        ) : null}
        {page === 'form' ? (
          <section className="pvC-card pv-stack" style={{ maxWidth: 720, margin: '0 auto', width: '100%' }}>
            <h1 style={{ fontSize: '1.4em' }}>Record vitals for {patient.name.split(' ')[1]}</h1>
            <ol className="pvC-steps" style={{ listStyle: 'none', padding: 0, margin: 0 }}>
              {['Measurements', 'Triage', 'Check & save'].map((s, i) => (
                <li key={s} className="pvC-step" aria-current={step === i + 1 ? 'step' : undefined}><span className="pvC-dot">{i + 1}</span>{s}</li>
              ))}
            </ol>
            {step === 1 ? <VitalsFields layout="stack" /> : null}
            {step === 2 ? (
              <fieldset className="pv-stack" style={{ border: 0, padding: 0, margin: 0 }}>
                <legend style={{ fontWeight: 600, marginBottom: 8 }}>How urgent is it?</legend>
                {(['red', 'yellow', 'green'] as Triage[]).map((t) => (
                  <label key={t} className="pvC-vital pv-row" style={{ cursor: 'pointer' }}>
                    <input type="radio" name="pvC-tri" defaultChecked={t === 'yellow'} /> <Triage t={t} />
                    <span className="pv-muted">{t === 'red' ? 'See within 10 minutes' : t === 'yellow' ? 'See within 1 hour' : 'Normal queue'}</span>
                  </label>
                ))}
              </fieldset>
            ) : null}
            {step === 3 ? <p>BP 118/76 mmHg · 36.8 °C · 64.2 kg · <Triage t="yellow" /></p> : null}
            <div className="pv-row" style={{ justifyContent: 'space-between' }}>
              <button className="pv-btn" onClick={() => (step === 1 ? setPage('chart') : setStep(step - 1))}>{step === 1 ? 'Cancel' : '← Back'}</button>
              <button className="pv-btn pv-solid" onClick={() => (step === 3 ? setPage('chart') : setStep(step + 1))}>{step === 3 ? 'Save vitals' : 'Next →'}</button>
            </div>
          </section>
        ) : null}
      </main>
      <EndVisitDialog d={dlg} patient={patient} />
    </div>
  )
}

// --- Switcher ----------------------------------------------------------------------------------------------

const THEMES: Theme[] = ['light', 'dark', 'contrast']
export function VisualDirectionPrototype() {
  const search = useSearch({ from: '/prototype/visual-direction' })
  const navigate = useNavigate({ from: '/prototype/visual-direction' })
  const idx = Math.max(0, VARIANTS.findIndex((v) => v.key === search.variant))
  const v = VARIANTS[idx] as (typeof VARIANTS)[number]
  const theme = (THEMES as string[]).includes(search.theme ?? '') ? (search.theme as Theme) : 'light'
  const density: Density = search.density === 'compact' ? 'compact' : 'comfortable'
  const [pid, setPid] = useState('CF-0041-2291')
  const patient = PATIENTS.find((p) => p.id === pid) ?? (PATIENTS[0] as Pt)
  const set = (p: Partial<{ variant: string; theme: string; density: string }>) =>
    void navigate({ search: (s) => ({ ...s, ...p }), replace: true })
  const go = (d: number) => set({ variant: (VARIANTS[(idx + d + VARIANTS.length) % VARIANTS.length] as (typeof VARIANTS)[number]).key })

  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement
      if (t.closest('input, textarea, select, [contenteditable]')) return
      if (e.key === 'ArrowLeft') go(-1)
      if (e.key === 'ArrowRight') go(1)
    }
    window.addEventListener('keydown', on)
    return () => window.removeEventListener('keydown', on)
  })

  const body: Record<Key, ReactNode> = {
    A: <VariantA patient={patient} setPatient={setPid} />,
    B: <VariantB patient={patient} setPatient={setPid} />,
    C: <VariantC patient={patient} setPatient={setPid} />,
  }
  return (
    <>
      <style>{CSS}</style>
      <div className="pv" data-dir={v.key} data-theme={theme} data-density={density}>
        {body[v.key]}
      </div>
      {import.meta.env.DEV ? (
        <div className="pvbar" role="toolbar" aria-label="Prototype switcher">
          <button onClick={() => go(-1)} aria-label="Previous direction">←</button>
          <span className="lbl" aria-live="polite">{v.key} ({v.name})</span>
          <button onClick={() => go(1)} aria-label="Next direction">→</button>
          <span className="sep" />
          {THEMES.map((t) => (
            <button key={t} aria-pressed={theme === t} onClick={() => set({ theme: t })}>{t === 'contrast' ? 'High contrast' : t[0]?.toUpperCase() + t.slice(1)}</button>
          ))}
          <span className="sep" />
          <button aria-pressed={density === 'compact'} onClick={() => set({ density: density === 'compact' ? 'comfortable' : 'compact' })}>
            {density === 'compact' ? 'Compact 32px' : 'Comfortable 44px'}
          </button>
        </div>
      ) : null}
    </>
  )
}
