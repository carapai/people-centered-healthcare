// PROTOTYPE (#482), throwaway: three structurally different Program Designers on
// /prototype/program-designer, switched with ?variant=A|B|C and the floating bar (← / → keys).
// A: one page per program, a section per §44 element, each listing the artefacts it links.
// B: the patient's journey as a board of stages, with a sample patient walked through it.
// C: a step-by-step assembly, with the package's contents and its gaps beside it.
// All three show the same in-memory ART package; editors open in their own screens (not here).
import { css, cx } from '@health/design-system/css'
import { useEffect, useState } from 'react'

import { ART, parts, problems, walk, type Patient, type Program, type Ref } from './programData'

const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const input = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', px: '2', minHeight: 'control', width: '100%', bg: 'bg.surface' })
const ghost = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', fontSize: 'control', cursor: 'pointer', border: '1px solid', borderColor: 'border', bg: 'bg.surface', _disabled: { opacity: 0.5, cursor: 'not-allowed' } })
const h2 = css({ fontSize: 'lg', fontWeight: 'semibold' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs' })
const bad = css({ color: 'danger.text' })
const ok = css({ color: 'success.text' })

function useProgram() {
  const [program, setProgram] = useState<Program>(ART)
  const [opened, setOpened] = useState<string | null>(null)
  const moveRegimen = (id: string, by: number) =>
    setProgram((p) => {
      const list = [...p.regimens]
      const at = list.findIndex((r) => r.id === id)
      const to = at + by
      if (to < 0 || to >= list.length) return p
      const a = list[at]
      const b = list[to]
      if (!a || !b) return p
      list[at] = b
      list[to] = a
      return { ...p, regimens: list }
    })
  const create = (ref: Ref) => {
    const mark = (r: Ref): Ref => (r.url === ref.url ? { ...r, where: 'draft' } : r)
    setProgram((p) => ({
      ...p,
      enrolment: { form: mark(p.enrolment.form), workflow: mark(p.enrolment.workflow) },
      stages: p.stages.map((s) => (s.form ? { ...s, form: mark(s.form) } : s)),
      regimens: p.regimens.map((r) => ({ ...r, activity: mark(r.activity) })),
      investigations: p.investigations.map((i) => ({ ...i, activity: mark(i.activity) })),
      indicators: p.indicators.map(mark),
    }))
    setOpened(`${ref.kind} “${ref.title}” created in this draft and opened in its editor`)
  }
  const open = (ref: Ref) => setOpened(`${ref.kind} “${ref.title}” would open in ${ref.kind === 'Questionnaire' ? 'the form designer' : ref.kind === 'Workflow Definition' ? 'the Workflow Studio' : 'its editor'}`)
  return { program, setProgram, moveRegimen, create, open, opened, setOpened }
}
type Model = ReturnType<typeof useProgram>

/** One linked artefact: what it is, where it stands, and the way into its editor. */
function Link({ model, r }: { model: Model; r: Ref }) {
  const tone = r.where === 'missing' ? bad : r.where === 'draft' ? css({ color: 'accent.default' }) : ok
  return (
    <span className={css({ display: 'inline-flex', gap: '2', alignItems: 'baseline', flexWrap: 'wrap' })}>
      <span>{r.title}</span>
      <span className={cx(css({ fontSize: 'xs' }), tone)}>{r.where === 'missing' ? 'not created' : r.where === 'draft' ? `in this draft, ${r.version}` : `published ${r.version}`}</span>
      {r.where === 'missing' ? (
        <button type="button" className={cx(ghost, css({ minHeight: 'auto', py: '0', fontSize: 'xs' }))} onClick={() => model.create(r)}>
          Create
        </button>
      ) : (
        <button type="button" className={cx(ghost, css({ minHeight: 'auto', py: '0', fontSize: 'xs' }))} onClick={() => model.open(r)}>
          Open
        </button>
      )}
    </span>
  )
}

function Regimens({ model }: { model: Model }) {
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <p className={muted}>The first rule that matches the patient chooses the first-line regimen, from the top down.</p>
      <ol className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {model.program.regimens.map((r, n) => (
          <li key={r.id} className={cx(box, css({ p: '2', display: 'flex', gap: '2', alignItems: 'flex-start' }))}>
            <div className={css({ display: 'flex', flexDirection: 'column' })}>
              <button type="button" aria-label={`Move ${r.title} up`} className={css({ cursor: 'pointer' })} disabled={n === 0} onClick={() => model.moveRegimen(r.id, -1)}>
                ▲
              </button>
              <button type="button" aria-label={`Move ${r.title} down`} className={css({ cursor: 'pointer' })} disabled={n === model.program.regimens.length - 1} onClick={() => model.moveRegimen(r.id, 1)}>
                ▼
              </button>
            </div>
            <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
              <strong>
                {n + 1}. {r.title} <span className={muted}>({r.line} line)</span>
              </strong>
              <span className={mono}>when {r.when.fhirpath}</span>
              <Link model={model} r={r.activity} />
              <span className={muted}>Source: {r.source}</span>
            </div>
          </li>
        ))}
      </ol>
    </div>
  )
}

const Gaps = ({ program }: { program: Program }) => {
  const found = problems(program)
  return found.length === 0 ? (
    <p className={ok}>Nothing missing. Check now on the draft page runs the full validation.</p>
  ) : (
    <ul className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '1' })}>
      {found.map((p) => (
        <li key={p} className={bad}>
          {p}
        </li>
      ))}
    </ul>
  )
}

// --- Variant A: one page, a section per element -------------------------------------------------

function VariantA({ model }: { model: Model }) {
  const p = model.program
  const sections: { id: string; title: string; done: boolean; body: React.ReactNode }[] = [
    {
      id: 'identity',
      title: 'Identity and eligibility',
      done: true,
      body: (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' })}>
          <span>EpisodeOfCare type: {p.episodeType}</span>
          <span>Approval: {p.approvers}</span>
          <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
            Eligible when (FHIRPath)
            <textarea className={cx(input, mono, css({ minHeight: '3rem', py: '1' }))} value={p.eligibility.fhirpath} onChange={(e) => model.setProgram({ ...p, eligibility: { ...p.eligibility, fhirpath: e.target.value } })} />
          </label>
        </div>
      ),
    },
    {
      id: 'enrolment',
      title: 'Enrolment',
      done: true,
      body: (
        <ul className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '1' })}>
          <li>
            Form: <Link model={model} r={p.enrolment.form} />
          </li>
          <li>
            Workflow: <Link model={model} r={p.enrolment.workflow} />
          </li>
        </ul>
      ),
    },
    {
      id: 'stages',
      title: 'Stages and their forms',
      done: p.stages.every((s) => s.form?.where !== 'missing'),
      body: (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' })}>
          <span>
            The stages are the states of <Link model={model} r={p.workflow} />. Changing them opens the Workflow Studio.
          </span>
          <table className={css({ width: '100%', '& td, & th': { textAlign: 'left', p: '1', borderBottom: '1px solid', borderColor: 'border', verticalAlign: 'top' } })}>
            <thead>
              <tr>
                <th>Stage</th>
                <th>Form</th>
                <th>What happens</th>
                <th>Leaves when</th>
              </tr>
            </thead>
            <tbody>
              {p.stages.map((s) => (
                <tr key={s.id}>
                  <td>{s.label}</td>
                  <td>{s.form ? <Link model={model} r={s.form} /> : '—'}</td>
                  <td>{s.does.join('; ')}</td>
                  <td>{s.leaves}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ),
    },
    { id: 'regimens', title: 'Regimen templates', done: p.regimens.every((r) => r.activity.where !== 'missing'), body: <Regimens model={model} /> },
    {
      id: 'investigations',
      title: 'Investigations',
      done: true,
      body: (
        <ul className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '1' })}>
          {p.investigations.map((i) => (
            <li key={i.id}>
              {i.title}: at month {i.at.join(' and ')}
              {i.later ? `, then ${i.later}` : ''} · <Link model={model} r={i.activity} />
            </li>
          ))}
        </ul>
      ),
    },
    {
      id: 'schedule',
      title: 'Follow-up schedule',
      done: true,
      body: (
        <div className={css({ fontSize: 'sm' })}>
          <p>{p.scheduleNote}</p>
          <p className={muted}>Visits: {p.schedule.map((v) => `${v.label} (day ${v.day})`).join(' · ')}</p>
        </div>
      ),
    },
    {
      id: 'outcomes',
      title: 'Outcomes',
      done: true,
      body: (
        <ul className={css({ fontSize: 'sm' })}>
          {p.outcomes.map((o) => (
            <li key={o.code}>
              <strong>{o.title}</strong>: {o.how}
            </li>
          ))}
        </ul>
      ),
    },
    {
      id: 'indicators',
      title: 'Indicators',
      done: p.indicators.every((i) => i.where !== 'missing'),
      body: (
        <ul className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '1' })}>
          {p.indicators.map((i) => (
            <li key={i.url}>
              <Link model={model} r={i} />
            </li>
          ))}
        </ul>
      ),
    },
    {
      id: 'applies',
      title: 'Where it applies',
      done: p.scopes.length > 0,
      body: (
        <div className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '1' })}>
          <span>Activated for: {p.scopes.join(', ')}</span>
          <span>What a facility admin may adjust:</span>
          <ul className={css({ pl: '4' })}>
            {p.overrides.map((o) => (
              <li key={o.name}>
                {o.name}: default {o.value}; allowed {o.allowed}
              </li>
            ))}
          </ul>
        </div>
      ),
    },
  ]
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '14rem 1fr' }, gap: '4', alignItems: 'start' })}>
      <nav aria-label="Sections" className={cx(box, css({ p: '2', position: { lg: 'sticky' }, top: '4' }))}>
        <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontSize: 'sm' })}>
          {sections.map((s) => (
            <li key={s.id}>
              <a href={`#${s.id}`} className={css({ display: 'block', px: '2', py: '1', borderRadius: 'sm', _hover: { bg: 'bg.subtle' } })}>
                {s.done ? <span className={ok}>✓</span> : <span className={bad}>!</span>} {s.title}
              </a>
            </li>
          ))}
        </ul>
      </nav>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        {sections.map((s) => (
          <section key={s.id} id={s.id} className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2' }))}>
            <h2 className={h2}>{s.title}</h2>
            {s.body}
          </section>
        ))}
      </div>
    </div>
  )
}

// --- Variant B: the journey as a board, with a sample patient walked through ---------------------

function VariantB({ model }: { model: Model }) {
  const p = model.program
  const [patient, setPatient] = useState<Patient>({ age: 34, weightKg: 58, sex: 'female', pregnant: false, hivPositive: true, onTbTreatment: true })
  const result = walk(p, patient, new Date('2026-11-17'))
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <div className={cx(box, css({ p: '3', display: 'flex', gap: '6', flexWrap: 'wrap', fontSize: 'sm' }))}>
        <span>
          <strong>Who:</strong> <span className={mono}>{p.eligibility.fhirpath}</span>
        </span>
        <span>
          <strong>Runs on:</strong> <Link model={model} r={p.workflow} />
        </span>
        <span>
          <strong>Where:</strong> {p.scopes.join(', ')} · {p.overrides.length} facility adjustments
        </span>
      </div>
      <div className={css({ display: 'grid', gridAutoFlow: 'column', gridAutoColumns: 'minmax(15rem, 1fr)', gap: '3', overflowX: 'auto', pb: '2' })}>
        {p.stages.map((s, n) => (
          <section key={s.id} className={cx(box, css({ p: '3', display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' }))}>
            <h2 className={h2}>
              {n + 1}. {s.label}
            </h2>
            {s.form ? (
              <div>
                <div className={muted}>Form</div>
                <Link model={model} r={s.form} />
              </div>
            ) : null}
            <div>
              <div className={muted}>What happens</div>
              <ul className={css({ pl: '4', listStyle: 'disc' })}>
                {s.does.map((d) => (
                  <li key={d}>{d}</li>
                ))}
              </ul>
            </div>
            {s.id === 'initiation' ? (
              <details>
                <summary className={css({ cursor: 'pointer' })}>Regimen templates ({p.regimens.length})</summary>
                <Regimens model={model} />
              </details>
            ) : null}
            {s.id === 'baseline' || s.id === 'follow-up' ? (
              <div>
                <div className={muted}>Investigations</div>
                {p.investigations
                  .filter((i) => (s.id === 'baseline' ? i.at.includes(0) : i.at.some((m) => m > 0)))
                  .map((i) => (
                    <div key={i.id}>
                      <Link model={model} r={i.activity} />
                    </div>
                  ))}
              </div>
            ) : null}
            {s.id === 'follow-up' ? (
              <div>
                <div className={muted}>Schedule</div>
                {p.scheduleNote}
              </div>
            ) : null}
            {s.id === 'outcome' ? (
              <div>
                <div className={muted}>Outcomes and indicators</div>
                {p.outcomes.map((o) => o.title).join(', ')}
                {p.indicators.map((i) => (
                  <div key={i.url}>
                    <Link model={model} r={i} />
                  </div>
                ))}
              </div>
            ) : null}
            <div className={css({ mt: 'auto', pt: '2', borderTop: '1px solid', borderColor: 'border' })}>
              <span className={muted}>Leaves when</span> {s.leaves}
            </div>
          </section>
        ))}
      </div>
      <section aria-label="Walk a patient through" className={cx(box, css({ p: '3', display: 'grid', gridTemplateColumns: { base: '1fr', lg: '18rem 1fr' }, gap: '4' }))}>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' })}>
          <h2 className={h2}>Walk a patient through</h2>
          <label>
            Age <input type="number" className={input} value={patient.age} onChange={(e) => setPatient({ ...patient, age: Number(e.target.value) })} />
          </label>
          <label>
            Weight (kg) <input type="number" className={input} value={patient.weightKg} onChange={(e) => setPatient({ ...patient, weightKg: Number(e.target.value) })} />
          </label>
          <label>
            <input type="checkbox" checked={patient.hivPositive} onChange={(e) => setPatient({ ...patient, hivPositive: e.target.checked })} /> HIV positive
          </label>
          <label>
            <input type="checkbox" checked={patient.onTbTreatment} onChange={(e) => setPatient({ ...patient, onTbTreatment: e.target.checked })} /> On TB treatment (rifampicin)
          </label>
          <label>
            <input type="checkbox" checked={patient.pregnant} onChange={(e) => setPatient({ ...patient, pregnant: e.target.checked })} /> Pregnant
          </label>
          <span className={muted}>The rules run in the server's FHIRPath engine; this prototype only imitates them.</span>
        </div>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' })}>
          <p>{result.eligible ? <span className={ok}>Eligible: enrols into the program.</span> : <span className={bad}>Not eligible: the eligibility rule is false.</span>}</p>
          {result.eligible ? (
            <p>
              First-line regimen: <strong>{result.regimen?.title ?? 'none: no rule matches this patient'}</strong>
              {result.regimen ? <span className={muted}> (rule {p.regimens.indexOf(result.regimen) + 1}, {result.regimen.when.fhirpath})</span> : null}
            </p>
          ) : null}
          <ol className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
            {result.visits.map((v) => (
              <li key={v.label} className={css({ display: 'grid', gridTemplateColumns: '7rem 12rem 1fr', gap: '2' })}>
                <span className={mono}>{v.date}</span>
                <span>{v.label}</span>
                <span className={muted}>{v.tests.join(', ')}</span>
              </li>
            ))}
          </ol>
        </div>
      </section>
    </div>
  )
}

// --- Variant C: assembled step by step, with the package beside it -------------------------------

const STEPS = ['Who it is for', 'Enrolment', 'Stages and forms', 'Regimens', 'Investigations and schedule', 'Outcomes and indicators', 'Where it applies'] as const

function VariantC({ model }: { model: Model }) {
  const p = model.program
  const [step, setStep] = useState(0)
  const all = parts(p)
  const folder = (r: Ref) => ({ Questionnaire: 'questionnaires/', 'Workflow Definition': 'workflows/', ActivityDefinition: 'activitydefinitions/', Measure: 'measures/', ValueSet: 'valuesets/', 'Extraction Mapping': 'mappings/' })[r.kind]
  const folders = [...new Set(all.map((x) => folder(x.ref)))]
  const body: React.ReactNode[] = [
    <div key="who" className={css({ display: 'flex', flexDirection: 'column', gap: '2', fontSize: 'sm' })}>
      <span>Program: {p.title}</span>
      <span>EpisodeOfCare type: {p.episodeType}</span>
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        Eligible when (FHIRPath)
        <textarea className={cx(input, mono, css({ minHeight: '3rem', py: '1' }))} value={p.eligibility.fhirpath} onChange={(e) => model.setProgram({ ...p, eligibility: { ...p.eligibility, fhirpath: e.target.value } })} />
      </label>
    </div>,
    <ul key="enrol" className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '2' })}>
      <li>
        Form: <Link model={model} r={p.enrolment.form} />
      </li>
      <li>
        Workflow: <Link model={model} r={p.enrolment.workflow} />
      </li>
    </ul>,
    <ul key="stages" className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '2' })}>
      <li>
        Program workflow: <Link model={model} r={p.workflow} />
      </li>
      {p.stages.map((s) => (
        <li key={s.id}>
          {s.label}: {s.form ? <Link model={model} r={s.form} /> : 'no form'}
        </li>
      ))}
    </ul>,
    <Regimens key="reg" model={model} />,
    <div key="inv" className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '2' })}>
      {p.investigations.map((i) => (
        <div key={i.id}>
          {i.title} at month {i.at.join(' and ')}
          {i.later ? `, then ${i.later}` : ''}: <Link model={model} r={i.activity} />
        </div>
      ))}
      <p>{p.scheduleNote}</p>
    </div>,
    <div key="out" className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '2' })}>
      {p.outcomes.map((o) => (
        <div key={o.code}>
          <strong>{o.title}</strong>: {o.how}
        </div>
      ))}
      {p.indicators.map((i) => (
        <div key={i.url}>
          <Link model={model} r={i} />
        </div>
      ))}
    </div>,
    <div key="where" className={css({ fontSize: 'sm', display: 'flex', flexDirection: 'column', gap: '2' })}>
      <span>Activated for: {p.scopes.join(', ')}</span>
      {p.overrides.map((o) => (
        <span key={o.name}>
          {o.name}: default {o.value}; a facility may choose {o.allowed}
        </span>
      ))}
    </div>,
  ]
  return (
    <div className={css({ display: 'grid', gridTemplateColumns: { base: '1fr', lg: '1fr 22rem' }, gap: '4', alignItems: 'start' })}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <ol className={css({ display: 'flex', gap: '1', flexWrap: 'wrap' })}>
          {STEPS.map((s, n) => (
            <li key={s}>
              <button type="button" aria-current={step === n ? 'step' : undefined} onClick={() => setStep(n)} className={cx(ghost, css({ fontSize: 'sm' }), step === n ? css({ bg: 'bg.selected', borderColor: 'accent.default' }) : undefined)}>
                {n + 1}. {s}
              </button>
            </li>
          ))}
        </ol>
        <section className={cx(box, css({ p: '4', display: 'flex', flexDirection: 'column', gap: '3', minHeight: '20rem' }))}>
          <h2 className={h2}>
            {step + 1}. {STEPS[step]}
          </h2>
          {body[step]}
        </section>
        <div className={css({ display: 'flex', justifyContent: 'space-between' })}>
          <button type="button" className={ghost} disabled={step === 0} onClick={() => setStep(step - 1)}>
            Back
          </button>
          <button type="button" className={ghost} disabled={step === STEPS.length - 1} onClick={() => setStep(step + 1)}>
            Next
          </button>
        </div>
      </div>
      <aside className={css({ display: 'flex', flexDirection: 'column', gap: '3', position: { lg: 'sticky' }, top: '4' })}>
        <section className={cx(box, css({ p: '3' }))}>
          <h2 className={h2}>
            {p.name} {p.version}
          </h2>
          <ul className={cx(mono, css({ mt: '2', display: 'flex', flexDirection: 'column', gap: '1' }))}>
            <li>package.json</li>
            {folders.map((f) => (
              <li key={f}>
                {f}
                <ul className={css({ pl: '4' })}>
                  {all
                    .filter((x) => folder(x.ref) === f)
                    .map((x) => (
                      <li key={x.ref.url} className={x.ref.where === 'missing' ? bad : x.ref.where === 'published' ? muted : undefined}>
                        {x.ref.url.split('/').pop()}
                        {x.ref.where === 'published' ? ` → ${x.ref.version} (published)` : x.ref.where === 'missing' ? ' (missing)' : ''}
                      </li>
                    ))}
                </ul>
              </li>
            ))}
            <li>tests/</li>
          </ul>
        </section>
        <section className={cx(box, css({ p: '3' }))}>
          <h2 className={h2}>Still missing</h2>
          <Gaps program={p} />
        </section>
      </aside>
    </div>
  )
}

// --- The shell -----------------------------------------------------------------------------

const VARIANTS = [
  { key: 'A', name: 'One page, a section per element', View: VariantA },
  { key: 'B', name: 'Journey board with a patient walked through', View: VariantB },
  { key: 'C', name: 'Step by step, with the package beside it', View: VariantC },
] as const

export function ProgramDesignerPrototype() {
  const model = useProgram()
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
  const gaps = problems(model.program)
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', pb: '16' })}>
      <header className={css({ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', flexWrap: 'wrap', gap: '3' })}>
        <div>
          <p className={muted}>
            ← {model.program.name} {model.program.version}, draft · Program
          </p>
          <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{model.program.title}</h1>
        </div>
        <span className={gaps.length ? bad : ok}>{gaps.length ? `${gaps.length} parts missing` : 'Every part exists'}</span>
      </header>
      {model.opened ? (
        <p role="status" className={cx(box, css({ p: '2', fontSize: 'sm', display: 'flex', justifyContent: 'space-between' }))}>
          {model.opened}
          <button type="button" className={css({ cursor: 'pointer' })} onClick={() => model.setOpened(null)}>
            ✕
          </button>
        </p>
      ) : null}
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
