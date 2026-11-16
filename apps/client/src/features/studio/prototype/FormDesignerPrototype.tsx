// PROTOTYPE (#479), throwaway: three structurally different form designers on
// /prototype/form-designer, switched with ?variant=A|B|C and the floating bar (← / → keys).
// A: outline, inspector and live preview side by side. B: the form as it will look, edited in
// place. C: a spreadsheet, one row per question, with the preview in a drawer.
// All three edit the same in-memory model; the preview is the real QuestionnaireRenderer.
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import { QuestionnaireRenderer } from '../../forms/QuestionnaireRenderer'
import { ITEM_TYPES, SAMPLE, WIDGETS, fromQuestionnaire, problems, toLayout, toMapping, toQuestionnaire, type Form, type Item } from './formDesignerData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', width: '100%' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface' })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })

type Edit = (linkId: string, change: Partial<Item>) => void

function useForm() {
  const [form, setForm] = useState<Form>(SAMPLE)
  const edit: Edit = (linkId, change) => setForm((f) => ({ ...f, items: f.items.map((i) => (i.linkId === linkId ? { ...i, ...change } : i)) }))
  const move = (linkId: string, by: number) =>
    setForm((f) => {
      const items = [...f.items]
      const at = items.findIndex((i) => i.linkId === linkId)
      const to = at + by
      if (to < 0 || to >= items.length) return f
      ;[items[at], items[to]] = [items[to] as Item, items[at] as Item]
      return { ...f, items }
    })
  const add = (section: string | null) =>
    setForm((f) => {
      const n = f.items.length + 1
      return { ...f, items: [...f.items, { linkId: `question-${n}`, text: `New question ${n}`, type: 'string', required: false, repeats: false, options: [], shownWhen: null, section, widget: null, hint: '', extract: { resource: 'none', code: '', display: '' } }] }
    })
  const remove = (linkId: string) => setForm((f) => ({ ...f, items: f.items.filter((i) => i.linkId !== linkId) }))
  return { form, setForm, edit, move, add, remove }
}

function Preview({ form }: { form: Form }) {
  const q = toQuestionnaire(form)
  // Remount on every change, so the renderer always shows the current definition.
  return <QuestionnaireRenderer key={JSON.stringify(q) + JSON.stringify(toLayout(form))} questionnaire={q} layout={toLayout(form)} onSubmit={() => undefined} />
}

/** Every property of one question, as one panel. */
function Inspector({ form, item, edit }: { form: Form; item: Item; edit: Edit }) {
  const booleans = form.items.filter((i) => i.type === 'boolean' && i.linkId !== item.linkId)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
      <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <legend className={css({ fontWeight: 'semibold' })}>Question (clinical, needs approval)</legend>
        <label className={label}>
          Text
          <input className={input} value={item.text} onChange={(e) => edit(item.linkId, { text: e.target.value })} />
        </label>
        <label className={label}>
          Id <span className={muted}>Fixed once published: answers and mappings refer to it</span>
          <input className={input} value={item.linkId} readOnly />
        </label>
        <label className={label}>
          Answer type
          <select className={input} value={item.type} onChange={(e) => edit(item.linkId, { type: e.target.value as Item['type'] })}>
            {ITEM_TYPES.map((t) => (
              <option key={t}>{t}</option>
            ))}
          </select>
        </label>
        <label>
          <input type="checkbox" checked={item.required} onChange={(e) => edit(item.linkId, { required: e.target.checked })} /> Required
        </label>
        {item.type === 'choice' ? (
          <>
            <label>
              <input type="checkbox" checked={item.repeats} onChange={(e) => edit(item.linkId, { repeats: e.target.checked })} /> More than one answer
            </label>
            <div className={label}>
              Options <span className={muted}>Or bind a published ValueSet (not in the prototype)</span>
              {item.options.map((o, n) => (
                <input key={o.code} className={input} value={o.display} onChange={(e) => edit(item.linkId, { options: item.options.map((x, m) => (m === n ? { ...x, display: e.target.value } : x)) })} />
              ))}
              <button type="button" className={ghost} onClick={() => edit(item.linkId, { options: [...item.options, { code: `option-${item.options.length + 1}`, display: `Option ${item.options.length + 1}` }] })}>
                Add option
              </button>
            </div>
          </>
        ) : null}
        <label className={label}>
          Shown only when
          <select className={input} value={item.shownWhen ?? ''} onChange={(e) => edit(item.linkId, { shownWhen: e.target.value === '' ? null : e.target.value })}>
            <option value="">Always shown</option>
            {booleans.map((b) => (
              <option key={b.linkId} value={b.linkId}>
                "{b.text}" is Yes
              </option>
            ))}
          </select>
        </label>
      </fieldset>
      <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <legend className={css({ fontWeight: 'semibold' })}>Layout (presentation)</legend>
        <label className={label}>
          Section
          <select className={input} value={item.section ?? ''} onChange={(e) => edit(item.linkId, { section: e.target.value === '' ? null : e.target.value })}>
            <option value="">No section (shown at the end)</option>
            {form.sections.map((s) => (
              <option key={s}>{s}</option>
            ))}
          </select>
        </label>
        <label className={label}>
          Shown as
          <select className={input} value={item.widget ?? ''} onChange={(e) => edit(item.linkId, { widget: e.target.value === '' ? null : (e.target.value as Item['widget']) })}>
            <option value="">Default for the type</option>
            {WIDGETS.map((w) => (
              <option key={w}>{w}</option>
            ))}
          </select>
        </label>
        <label className={label}>
          Hint
          <input className={input} value={item.hint} onChange={(e) => edit(item.linkId, { hint: e.target.value })} />
        </label>
      </fieldset>
      <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <legend className={css({ fontWeight: 'semibold' })}>Becomes a record</legend>
        <select className={input} value={item.extract.resource} onChange={(e) => edit(item.linkId, { extract: { ...item.extract, resource: e.target.value as Item['extract']['resource'] } })}>
          <option value="none">Nothing (kept on the form only)</option>
          <option>Observation</option>
          <option>Condition</option>
        </select>
        {item.extract.resource === 'none' ? null : (
          <label className={label}>
            Code (LOINC / local)
            <input className={input} value={item.extract.code} onChange={(e) => edit(item.linkId, { extract: { ...item.extract, code: e.target.value } })} />
          </label>
        )}
      </fieldset>
    </div>
  )
}

// A: outline on the left, the selected question's inspector in the middle, the live preview right.
function VariantA({ model }: { model: ReturnType<typeof useForm> }) {
  const { form, edit, move, add } = model
  const [selected, setSelected] = useState(form.items[0]?.linkId ?? '')
  const item = form.items.find((i) => i.linkId === selected)
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: '16rem 22rem 1fr', gap: '3', alignItems: 'start' })}>
      <nav aria-label="Questions" className={cx(box, css({ p: '2', display: 'flex', flexDirection: 'column', gap: '1' }))}>
        {[...form.sections, null].map((s) => (
          <div key={s ?? 'none'}>
            <div className={cx(muted, css({ mt: '2', fontWeight: 'semibold' }))}>{s ?? 'No section'}</div>
            {form.items
              .filter((i) => i.section === s)
              .map((i) => (
                <div key={i.linkId} className={css({ display: 'flex', alignItems: 'center', gap: '1' })}>
                  <button type="button" onClick={() => setSelected(i.linkId)} className={css({ flex: '1', textAlign: 'left', p: '1', borderRadius: 'sm', bg: i.linkId === selected ? 'bg.selected' : 'transparent', cursor: 'pointer' })}>
                    {i.shownWhen === null ? '' : '↳ '}
                    {i.text} <span className={muted}>{i.type}</span>
                  </button>
                  <button type="button" aria-label="Up" onClick={() => move(i.linkId, -1)}>
                    ↑
                  </button>
                  <button type="button" aria-label="Down" onClick={() => move(i.linkId, 1)}>
                    ↓
                  </button>
                </div>
              ))}
          </div>
        ))}
        <button type="button" className={ghost} onClick={() => add(item?.section ?? null)}>
          Add question
        </button>
      </nav>
      <section className={cx(box, css({ p: '3' }))}>{item === undefined ? <p className={muted}>Choose a question.</p> : <Inspector form={form} item={item} edit={edit} />}</section>
      <section className={cx(box, css({ p: '4' }))}>
        <h2 className={cx(muted, css({ mb: '2' }))}>Live preview: exactly what a health worker sees</h2>
        <Preview form={form} />
      </section>
    </div>
  )
}

// B: the form as it will look, section by section; clicking a question opens its editor in place.
// "Try it" swaps in the real renderer to fill it in.
function VariantB({ model }: { model: ReturnType<typeof useForm> }) {
  const { form, edit, move, add, remove } = model
  const [open, setOpen] = useState<string | null>(null)
  const [trying, setTrying] = useState(false)
  if (trying)
    return (
      <div className={css({ maxWidth: '3xl', display: 'flex', flexDirection: 'column', gap: '3' })}>
        <button type="button" className={ghost} onClick={() => setTrying(false)}>
          Back to editing
        </button>
        <Preview form={form} />
      </div>
    )
  return (
    <div className={css({ maxWidth: '3xl', display: 'flex', flexDirection: 'column', gap: '4' })}>
      <div className={css({ display: 'flex', gap: '2' })}>
        <button type="button" className={ghost} onClick={() => setTrying(true)}>
          Try it: fill the form in
        </button>
      </div>
      {[...form.sections, null].map((s) => (
        <section key={s ?? 'none'} className={cx(box, css({ p: '4', display: 'flex', flexDirection: 'column', gap: '2' }))}>
          <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{s ?? 'Not in a section (shown last)'}</h2>
          {form.items
            .filter((i) => i.section === s)
            .map((i) => (
              <div key={i.linkId} className={css({ borderBottom: '1px solid', borderColor: 'border', pb: '2' })}>
                <button type="button" onClick={() => setOpen(open === i.linkId ? null : i.linkId)} className={css({ width: '100%', textAlign: 'left', cursor: 'pointer', display: 'flex', justifyContent: 'space-between', gap: '2' })}>
                  <span>
                    <strong>
                      {i.text}
                      {i.required ? ' *' : ''}
                    </strong>
                    {i.shownWhen === null ? null : <span className={muted}> · only if "{form.items.find((x) => x.linkId === i.shownWhen)?.text}" is Yes</span>}
                    <div className={muted}>
                      {i.type === 'choice' ? i.options.map((o) => o.display).join(' / ') : `[${i.type}]`} {i.hint === '' ? '' : `· ${i.hint}`}
                    </div>
                  </span>
                  {i.extract.resource === 'none' ? null : (
                    <span className={css({ fontSize: 'xs', border: '1px solid', borderColor: 'border', borderRadius: 'full', px: '2', alignSelf: 'start', whiteSpace: 'nowrap' })}>
                      → {i.extract.resource} {i.extract.code}
                    </span>
                  )}
                </button>
                {open === i.linkId ? (
                  <div className={css({ mt: '3', p: '3', bg: 'bg.subtle', borderRadius: 'md', display: 'flex', flexDirection: 'column', gap: '2' })}>
                    <div className={css({ display: 'flex', gap: '2' })}>
                      <button type="button" className={ghost} onClick={() => move(i.linkId, -1)}>
                        Move up
                      </button>
                      <button type="button" className={ghost} onClick={() => move(i.linkId, 1)}>
                        Move down
                      </button>
                      <button type="button" className={ghost} onClick={() => remove(i.linkId)}>
                        Remove
                      </button>
                    </div>
                    <Inspector form={form} item={i} edit={edit} />
                  </div>
                ) : null}
              </div>
            ))}
          <button type="button" className={ghost} onClick={() => add(s)}>
            Add a question here
          </button>
        </section>
      ))}
    </div>
  )
}

// C: a spreadsheet, one row per question and a column per property (as XLSForm or DHIS2 data
// managers work); the preview opens in a drawer at the bottom.
function VariantC({ model }: { model: ReturnType<typeof useForm> }) {
  const { form, edit, add } = model
  const [preview, setPreview] = useState(false)
  const cell = css({ border: '1px solid', borderColor: 'border', px: '1', minHeight: '2rem', width: '100%', bg: 'transparent' })
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3', pb: preview ? '24rem' : '16' })}>
      <div className={css({ overflowX: 'auto' })}>
        <table className={css({ borderCollapse: 'collapse', fontSize: 'sm', '& th': { textAlign: 'left', p: '1', bg: 'bg.subtle', border: '1px solid', borderColor: 'border', whiteSpace: 'nowrap' }, '& td': { p: '0', border: '1px solid', borderColor: 'border' } })}>
          <thead>
            <tr>
              <th>Id</th>
              <th>Question</th>
              <th>Type</th>
              <th>Req.</th>
              <th>Options</th>
              <th>Shown only when</th>
              <th>Section</th>
              <th>Shown as</th>
              <th>Becomes</th>
              <th>Code</th>
            </tr>
          </thead>
          <tbody>
            {form.items.map((i) => (
              <tr key={i.linkId}>
                <td className={cx(muted, css({ px: '1' }))}>{i.linkId}</td>
                <td>
                  <input className={cell} value={i.text} onChange={(e) => edit(i.linkId, { text: e.target.value })} />
                </td>
                <td>
                  <select className={cell} value={i.type} onChange={(e) => edit(i.linkId, { type: e.target.value as Item['type'] })}>
                    {ITEM_TYPES.map((t) => (
                      <option key={t}>{t}</option>
                    ))}
                  </select>
                </td>
                <td className={css({ textAlign: 'center' })}>
                  <input type="checkbox" checked={i.required} onChange={(e) => edit(i.linkId, { required: e.target.checked })} />
                </td>
                <td>
                  <input
                    className={cell}
                    disabled={i.type !== 'choice'}
                    value={i.options.map((o) => o.display).join('; ')}
                    onChange={(e) =>
                      edit(i.linkId, {
                        options: e.target.value
                          .split(';')
                          .map((d) => d.trim())
                          .filter((d) => d !== '')
                          .map((d, n) => ({ code: i.options[n]?.code ?? d.toLowerCase().replace(/\W+/g, '-'), display: d })),
                      })
                    }
                  />
                </td>
                <td>
                  <select className={cell} value={i.shownWhen ?? ''} onChange={(e) => edit(i.linkId, { shownWhen: e.target.value === '' ? null : e.target.value })}>
                    <option value="">always</option>
                    {form.items
                      .filter((b) => b.type === 'boolean' && b.linkId !== i.linkId)
                      .map((b) => (
                        <option key={b.linkId} value={b.linkId}>
                          {b.linkId} = yes
                        </option>
                      ))}
                  </select>
                </td>
                <td>
                  <select className={cell} value={i.section ?? ''} onChange={(e) => edit(i.linkId, { section: e.target.value === '' ? null : e.target.value })}>
                    <option value="">(end)</option>
                    {form.sections.map((s) => (
                      <option key={s}>{s}</option>
                    ))}
                  </select>
                </td>
                <td>
                  <select className={cell} value={i.widget ?? ''} onChange={(e) => edit(i.linkId, { widget: e.target.value === '' ? null : (e.target.value as Item['widget']) })}>
                    <option value="">default</option>
                    {WIDGETS.map((w) => (
                      <option key={w}>{w}</option>
                    ))}
                  </select>
                </td>
                <td>
                  <select className={cell} value={i.extract.resource} onChange={(e) => edit(i.linkId, { extract: { ...i.extract, resource: e.target.value as Item['extract']['resource'] } })}>
                    <option value="none">—</option>
                    <option>Observation</option>
                    <option>Condition</option>
                  </select>
                </td>
                <td>
                  <input className={cell} disabled={i.extract.resource === 'none'} value={i.extract.code} onChange={(e) => edit(i.linkId, { extract: { ...i.extract, code: e.target.value } })} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className={css({ display: 'flex', gap: '2' })}>
        <button type="button" className={ghost} onClick={() => add(null)}>
          Add row
        </button>
        <button type="button" className={ghost} onClick={() => setPreview(!preview)}>
          {preview ? 'Hide preview' : 'Show preview'}
        </button>
      </div>
      {preview ? (
        <section className={cx(box, css({ position: 'fixed', left: '0', right: '0', bottom: '0', height: '22rem', overflowY: 'auto', p: '4', boxShadow: 'lg', zIndex: '5' }))}>
          <Preview form={form} />
        </section>
      ) : null}
    </div>
  )
}

const VARIANTS = [
  { key: 'A', name: 'Outline, inspector, live preview', View: VariantA },
  { key: 'B', name: 'The form as it looks, edited in place', View: VariantB },
  { key: 'C', name: 'Spreadsheet with a preview drawer', View: VariantC },
] as const

export function FormDesignerPrototype() {
  const model = useForm()
  const [variant, setVariant] = useState(() => new URLSearchParams(window.location.search).get('variant') ?? 'A')
  const [importing, setImporting] = useState(false)
  const [pasted, setPasted] = useState('')
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
  const found = problems(model.form)
  const View = current.View
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', pb: '16' })}>
      <header className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', flexWrap: 'wrap', gap: '3' })}>
        <div>
          <p className={muted}>← ANC (ug.moh.anc 2.1.0, draft) · Questionnaire</p>
          <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{model.form.title}</h1>
        </div>
        <button type="button" className={ghost} onClick={() => setImporting(!importing)}>
          Import FHIR JSON
        </button>
      </header>
      {importing ? (
        <div className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
          <textarea className={css({ fontFamily: 'mono', minHeight: '8rem', border: '1px solid', borderColor: 'border', p: '2' })} placeholder='{"resourceType": "Questionnaire", …}' value={pasted} onChange={(e) => setPasted(e.target.value)} />
          <button
            type="button"
            className={ghost}
            onClick={() => {
              model.setForm(fromQuestionnaire(pasted, model.form))
              setImporting(false)
            }}
          >
            Replace this form with the pasted Questionnaire
          </button>
        </div>
      ) : null}
      {found.length === 0 ? null : (
        <ul className={css({ color: 'danger.text', fontSize: 'sm' })}>
          {found.map((p) => (
            <li key={p}>⚠ {p}</li>
          ))}
        </ul>
      )}
      <View model={model} />
      <details className={box}>
        <summary className={css({ p: '2', cursor: 'pointer' })}>What gets saved: Questionnaire, Form Layout, extraction</summary>
        <pre className={css({ fontFamily: 'mono', fontSize: 'xs', p: '3', overflowX: 'auto' })}>{JSON.stringify({ questionnaire: toQuestionnaire(model.form), layout: toLayout(model.form), extraction: toMapping(model.form) }, null, 2)}</pre>
      </details>
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
