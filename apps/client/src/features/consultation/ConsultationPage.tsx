import { css } from '@health/design-system/css'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type ReactNode } from 'react'
import { useIntl } from 'react-intl'

import { consultationMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import type { PatientSummary } from '../../platform/fhir'
import { newResourceId } from '../../platform/ids'
import { SERVICE_POINTS, patientName, queueKey, queueQuery, step, type QueueEntry } from '../reception/api'
import { button, control, field } from '../reception/styles'
import {
  ConsultationError,
  NOTE_SECTIONS,
  amendNote,
  linkToPlan,
  listPlans,
  plansKey,
  startPlan,
  dispose,
  draftNote,
  listNotes,
  notesKey,
  record,
  signNote,
  summaryKey,
  type ClinicalNote,
  type Command,
  type NoteKind,
} from './api'

const section = css({ display: 'flex', flexDirection: 'column', gap: '2', borderTop: '1px solid', borderColor: 'border', pt: '3' })
const heading = css({ fontSize: 'lg', fontWeight: 'semibold' })

/** Someone in the consultation, by their queue entry. */
interface Open {
  entry: QueueEntry
  name: string
}

/**
 * The consultation (#104; clinical-workspaces.md): the patients waiting at consultation, and for
 * the one being seen, everything on one page: the shared record's summary, notes, diagnoses,
 * test orders, prescriptions and where the patient goes next. Every write is a command the node
 * checks; nothing is assembled here.
 */
export function ConsultationPage() {
  const [open, setOpen] = useState<Open | undefined>()
  return open === undefined ? <Waiting onOpen={setOpen} /> : <Workspace open={open} onClose={() => setOpen(undefined)} />
}

function Waiting({ onOpen }: { onOpen: (open: Open) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const entries = useQuery({ ...queueQuery(platform.transport, 'consultation'), refetchInterval: 15_000 })
  const names = useQueries({
    queries: (entries.data ?? []).map((e) => ({
      queryKey: ['fhir', 'Patient', e.patient],
      queryFn: () => platform.fhir.read('Patient', e.patient.replace(/^Patient\//, '')),
      staleTime: 300_000,
    })),
  })
  const see = useMutation({
    mutationFn: async (entry: QueueEntry) => {
      if (entry.status === 'waiting') await step(platform.transport, entry.id, 'call')
      if (entry.status !== 'in-service') await step(platform.transport, entry.id, 'serve')
      return entry
    },
  })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.csTitle)}</h1>
      <h2 className={heading}>{intl.formatMessage(m.csWaiting)}</h2>
      {see.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.csFailed, { reason: '' })}
        </p>
      ) : null}
      {entries.data?.length === 0 ? <p role="status">{intl.formatMessage(m.csNoneWaiting)}</p> : null}
      <ol aria-label={intl.formatMessage(m.csWaiting)} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {(entries.data ?? []).map((entry, index) => {
          const name = patientName(names[index]?.data) || entry.patient
          return (
            <li key={entry.id}>
              <button type="button" className={button} disabled={see.isPending} onClick={() => see.mutate(entry, { onSuccess: () => onOpen({ entry, name }) })}>
                {intl.formatMessage(m.csSee, { name })}
              </button>
            </li>
          )
        })}
      </ol>
    </section>
  )
}

function SummarySection({ title, items, render }: { title: string; items: unknown[] | null | undefined; render: () => ReactNode }) {
  const intl = useIntl()
  return (
    <section aria-label={title} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <h3 className={css({ fontWeight: 'semibold' })}>{title}</h3>
      {items === null ? <p>{intl.formatMessage(m.csNotShown)}</p> : items === undefined || items.length === 0 ? <p>{intl.formatMessage(m.csNoneRecorded)}</p> : <ul>{render()}</ul>}
    </section>
  )
}

function Summary({ summary }: { summary: PatientSummary | undefined }) {
  const intl = useIntl()
  return (
    <section className={section}>
      <h2 className={heading}>{intl.formatMessage(m.csSummary)}</h2>
      <SummarySection
        title={intl.formatMessage(m.csAllergies)}
        items={summary?.allergies}
        render={() => summary?.allergies?.map((a) => <li key={a.reference}>{[a.substance.text, a.criticality].filter(Boolean).join(' · ')}</li>)}
      />
      <SummarySection
        title={intl.formatMessage(m.csConditions)}
        items={summary?.conditions}
        render={() => summary?.conditions?.map((c) => <li key={c.reference}>{[c.code.text, c.verification].filter(Boolean).join(' · ')}</li>)}
      />
      <SummarySection
        title={intl.formatMessage(m.csMedications)}
        items={summary?.medications}
        render={() => summary?.medications?.map((d) => <li key={d.reference}>{[d.medication.text, d.dosage].filter(Boolean).join(' · ')}</li>)}
      />
      <SummarySection
        title={intl.formatMessage(m.csVitals)}
        items={summary?.vitals}
        render={() => summary?.vitals?.map((v) => <li key={v.reference}>{[v.code.text, [v.value, v.unit].filter(Boolean).join(' ')].filter(Boolean).join(': ')}</li>)}
      />
      <SummarySection
        title={intl.formatMessage(m.csVisits)}
        items={summary?.visits}
        render={() => summary?.visits?.map((v) => <li key={v.reference}>{[v.start?.slice(0, 10), v.location, v.status].filter(Boolean).join(' · ')}</li>)}
      />
    </section>
  )
}

/** One command form: its fields, its button, and what came of it. */
function useCommand(encounter: string, patient: string) {
  const platform = usePlatform()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (command: Command) => record(platform.transport, encounter, command),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: summaryKey(patient) }),
  })
}

function Outcome({ mutation, done }: { mutation: { isSuccess: boolean; isError: boolean; error: unknown }; done: string }) {
  const intl = useIntl()
  if (mutation.isSuccess) return <p role="status">{done}</p>
  if (mutation.isError) {
    const reason = mutation.error instanceof ConsultationError ? mutation.error.reason || String(mutation.error.status) : ''
    return (
      <p role="alert" className={css({ color: 'danger.text' })}>
        {intl.formatMessage(m.csFailed, { reason })}
      </p>
    )
  }
  return null
}

/** A command's form, submitted without a page load. */
const form = (onSubmit: () => void, children: ReactNode) => (
  <form
    className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}
    onSubmit={(event) => {
      event.preventDefault()
      onSubmit()
    }}
  >
    {children}
  </form>
)

/** The note being written: a new one, or an amendment of a signed one. */
type Editing = { kind: NoteKind; amends?: ClinicalNote; texts: string[] }

const KINDS: NoteKind[] = ['progress', 'consultation', 'admission', 'procedure', 'discharge', 'letter']

/** A new note of `kind`, its sections empty. */
const fresh = (kind: NoteKind): Editing => ({ kind, texts: NOTE_SECTIONS[kind].map(() => '') })

/**
 * The patient's notes (#107): written in sections, saved as drafts or signed. A signed note is
 * never edited; it is amended, and the amendment relates to it.
 */
function NotesPanel({ patient, encounter }: { patient: string; encounter: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const kindId = useId()
  const notes = useQuery({ queryKey: notesKey(patient), queryFn: () => listNotes(platform.transport, patient) })
  const [editing, setEditing] = useState<Editing>({ kind: 'progress', texts: NOTE_SECTIONS.progress.map(() => '') })
  const [done, setDone] = useState('')
  const refresh = () => queryClient.invalidateQueries({ queryKey: notesKey(patient) })
  const titles = editing.amends?.sections.map((s) => s.title) ?? NOTE_SECTIONS[editing.kind]
  const sections = titles.map((title, i) => ({ title, text: editing.texts[i] ?? '' }))
  const empty = sections.every((s) => s.text.trim() === '')
  const kindName = (kind: NoteKind) => intl.formatMessage(m.csKind, { kind })

  const write = useMutation({
    mutationFn: async (how: 'draft' | 'sign') => {
      if (editing.amends !== undefined) {
        await amendNote(platform.transport, editing.amends.id, { id: newResourceId(), relation: 'replaces', sections })
        return m.csAmendmentSigned
      }
      const drafted = await draftNote(platform.transport, { id: newResourceId(), kind: editing.kind, patient: `Patient/${patient}`, encounter: `Encounter/${encounter}`, sections })
      if (how === 'sign') await signNote(platform.transport, drafted.id)
      return how === 'sign' ? m.csNoteSigned : m.csDraftSaved
    },
    onSuccess: (message) => {
      setDone(intl.formatMessage(message))
      setEditing(fresh('progress'))
    },
    onSettled: refresh,
  })
  const sign = useMutation({
    mutationFn: (id: string) => signNote(platform.transport, id),
    onSuccess: () => setDone(intl.formatMessage(m.csNoteSigned)),
    onSettled: refresh,
  })
  const failure = write.error ?? sign.error
  const reason = failure instanceof ConsultationError ? failure.reason || String(failure.status) : ''

  return (
    <section aria-labelledby={`${kindId}-notes`} className={section}>
      <h2 id={`${kindId}-notes`} className={heading}>
        {intl.formatMessage(m.csNotes)}
      </h2>
      {form(
        () => write.mutate('sign'),
        <>
          {editing.amends === undefined ? (
            <div className={field}>
              <label htmlFor={kindId}>{intl.formatMessage(m.csNoteKind)}</label>
              <select id={kindId} value={editing.kind} onChange={(e) => setEditing(fresh(e.target.value as NoteKind))} className={control}>
                {KINDS.map((k) => (
                  <option key={k} value={k}>
                    {kindName(k)}
                  </option>
                ))}
              </select>
            </div>
          ) : (
            <p role="status">{intl.formatMessage(m.csAmending, { kind: kindName(editing.amends.kind) })}</p>
          )}
          {sections.map((s, i) => (
            <SectionField
              key={`${editing.kind}-${s.title}`}
              title={s.title}
              value={s.text}
              onChange={(text) => setEditing({ ...editing, texts: titles.map((_, j) => (j === i ? text : (editing.texts[j] ?? ''))) })}
            />
          ))}
          <div className={css({ display: 'flex', gap: '2' })}>
            {editing.amends === undefined ? (
              <button type="button" className={button} disabled={write.isPending || empty} onClick={() => write.mutate('draft')}>
                {intl.formatMessage(m.csSaveDraft)}
              </button>
            ) : null}
            <button type="submit" className={button} disabled={write.isPending || empty}>
              {intl.formatMessage(editing.amends === undefined ? m.csSign : m.csSignAmendment)}
            </button>
          </div>
        </>,
      )}
      {done !== '' && failure === null ? <p role="status">{done}</p> : null}
      {failure !== null ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.csFailed, { reason })}
        </p>
      ) : null}
      {notes.data?.length === 0 ? <p>{intl.formatMessage(m.csNoNotes)}</p> : null}
      <ul aria-label={intl.formatMessage(m.csNotes)} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        {(notes.data ?? []).map((note) => (
          <li key={note.id} className={css({ borderLeft: '3px solid', borderColor: 'border', pl: '3' })}>
            <p className={css({ fontWeight: 'semibold' })}>
              {kindName(note.kind)} · {intl.formatMessage(m.csStatus, { status: note.status.replaceAll('-', '') })}
            </p>
            {note.signed === null ? null : (
              <p>{intl.formatMessage(m.csSignedBy, { name: note.signed.by, date: intl.formatDate(new Date(note.signed.at), { dateStyle: 'medium', timeStyle: 'short' }) })}</p>
            )}
            {note.relatesTo === null ? null : <p>{intl.formatMessage(m.csAmends)}</p>}
            <dl>
              {note.sections.map((s) => (
                <div key={s.title}>
                  <dt className={css({ fontWeight: 'semibold' })}>{s.title}</dt>
                  <dd className={css({ whiteSpace: 'pre-wrap' })}>{s.text}</dd>
                </div>
              ))}
            </dl>
            {note.status === 'preliminary' ? (
              <button type="button" className={button} disabled={sign.isPending} onClick={() => sign.mutate(note.id)}>
                {intl.formatMessage(m.csSignDraft, { kind: kindName(note.kind) })}
              </button>
            ) : null}
            {note.signed !== null && note.status !== 'entered-in-error' ? (
              <button type="button" className={button} onClick={() => setEditing({ kind: note.kind, amends: note, texts: note.sections.map((s) => s.text) })}>
                {intl.formatMessage(m.csAmend, { kind: kindName(note.kind) })}
              </button>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  )
}

function SectionField({ title, value, onChange }: { title: string; value: string; onChange: (text: string) => void }) {
  const id = useId()
  return (
    <div className={field}>
      <label htmlFor={id}>{title}</label>
      <textarea id={id} rows={2} value={value} onChange={(e) => onChange(e.target.value)} className={control} />
    </div>
  )
}

/**
 * The patient's care plans (#110): start one, then add the patient's prescriptions and orders as
 * its activities and their diagnoses as what it addresses.
 */
function CarePlansPanel({ patient, summary }: { patient: string; summary: PatientSummary | undefined }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { title: useId(), add: useId() }
  const plans = useQuery({ queryKey: plansKey(patient), queryFn: () => listPlans(platform.transport, patient) })
  const [title, setTitle] = useState('')
  const [chosen, setChosen] = useState<Record<string, string>>({})
  const [done, setDone] = useState('')
  const refresh = () => queryClient.invalidateQueries({ queryKey: plansKey(patient) })
  const start = useMutation({
    mutationFn: () => startPlan(platform.transport, newResourceId(), `Patient/${patient}`, title.trim()),
    onSuccess: () => {
      setTitle('')
      setDone(intl.formatMessage(m.cpStarted))
    },
    onSettled: refresh,
  })
  const link = useMutation({
    mutationFn: ({ plan, reference }: { plan: string; reference: string }) => linkToPlan(platform.transport, plan, reference),
    onSuccess: () => setDone(intl.formatMessage(m.cpAdded)),
    onSettled: refresh,
  })
  // What may be added: the patient's prescriptions and diagnoses, as the summary names them.
  const choices = [
    ...(summary?.medications ?? []).map((d) => ({ reference: d.reference, text: d.medication.text })),
    ...(summary?.conditions ?? []).map((c) => ({ reference: c.reference, text: c.code.text })),
  ]
  const named = (reference: string) => choices.find((c) => c.reference === reference)?.text ?? reference
  const failure = start.error ?? link.error
  return (
    <section aria-labelledby={`${ids.title}-plans`} className={section}>
      <h2 id={`${ids.title}-plans`} className={heading}>
        {intl.formatMessage(m.cpTitle)}
      </h2>
      {form(
        () => start.mutate(),
        <>
          <div className={field}>
            <label htmlFor={ids.title}>{intl.formatMessage(m.cpName)}</label>
            <input id={ids.title} value={title} onChange={(e) => setTitle(e.target.value)} className={control} />
          </div>
          <div>
            <button type="submit" className={button} disabled={start.isPending || title.trim() === ''}>
              {intl.formatMessage(m.cpStart)}
            </button>
          </div>
        </>,
      )}
      {done !== '' && failure === null ? <p role="status">{done}</p> : null}
      {failure !== null ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.csFailed, { reason: failure instanceof ConsultationError ? failure.reason || String(failure.status) : '' })}
        </p>
      ) : null}
      {plans.data?.length === 0 ? <p>{intl.formatMessage(m.cpNone)}</p> : null}
      <ul aria-label={intl.formatMessage(m.cpTitle)} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        {(plans.data ?? []).map((plan) => (
          <li key={plan.id} className={css({ borderLeft: '3px solid', borderColor: 'border', pl: '3' })}>
            <p className={css({ fontWeight: 'semibold' })}>
              {plan.title} · {intl.formatMessage(m.cpStatus, { status: plan.status.replaceAll('-', '') })}
            </p>
            {plan.addresses.map((c) => (
              <p key={c}>{intl.formatMessage(m.cpAddresses, { what: named(c) })}</p>
            ))}
            <ul>
              {plan.activities.map((a) => (
                <li key={a}>{named(a)}</li>
              ))}
            </ul>
            {plan.status === 'active' && choices.length > 0 ? (
              <div className={css({ display: 'flex', gap: '2', alignItems: 'end', flexWrap: 'wrap' })}>
                <div className={field}>
                  <label htmlFor={`${ids.add}-${plan.id}`}>{intl.formatMessage(m.cpAddTo, { title: plan.title })}</label>
                  <select
                    id={`${ids.add}-${plan.id}`}
                    value={chosen[plan.id] ?? choices[0]?.reference ?? ''}
                    onChange={(e) => setChosen({ ...chosen, [plan.id]: e.target.value })}
                    className={control}
                  >
                    {choices.map((c) => (
                      <option key={c.reference} value={c.reference}>
                        {c.text}
                      </option>
                    ))}
                  </select>
                </div>
                <button
                  type="button"
                  className={button}
                  disabled={link.isPending}
                  onClick={() => link.mutate({ plan: plan.id, reference: chosen[plan.id] ?? choices[0]?.reference ?? '' })}
                >
                  {intl.formatMessage(m.cpAdd)}
                </button>
              </div>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  )
}

function Workspace({ open, onClose }: { open: Open; onClose: () => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { diagnosis: useId(), provisional: useId(), test: useId(), medicine: useId(), dosage: useId(), next: useId() }
  const encounter = open.entry.encounter.replace(/^Encounter\//, '')
  const patient = open.entry.patient.replace(/^Patient\//, '')
  const summary = useQuery({ queryKey: summaryKey(patient), queryFn: ({ signal }) => platform.fhir.summary(patient, { signal }) })

  const [diagnosis, setDiagnosis] = useState('')
  const [provisional, setProvisional] = useState(false)
  const [test, setTest] = useState('')
  const [medicine, setMedicine] = useState('')
  const [dosage, setDosage] = useState('')
  const [next, setNext] = useState<string>(SERVICE_POINTS.find((s) => s !== 'consultation') ?? 'pharmacy')
  const [last, setLast] = useState({ diagnosis: '', test: '', medicine: '' })

  const diagnose = useCommand(encounter, patient)
  const order = useCommand(encounter, patient)
  const prescribe = useCommand(encounter, patient)
  const disposition = useMutation({
    mutationFn: (to: { complete: true } | { servicePoint: string }) => dispose(platform.transport, encounter, to),
    onSettled: () => queryClient.invalidateQueries({ queryKey: queueKey('consultation') }),
  })

  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.csWith, { name: open.name })}</h1>
      <Summary summary={summary.data} />

      <NotesPanel patient={patient} encounter={encounter} />

      <CarePlansPanel patient={patient} summary={summary.data} />

      <section className={section}>
        {form(
          () => {
            setLast({ ...last, diagnosis })
            diagnose.mutate({ what: 'diagnoses', body: { id: newResourceId(), concept: { display: diagnosis.trim() }, provisional } }, { onSuccess: () => setDiagnosis('') })
          },
          <>
            <div className={field}>
              <label htmlFor={ids.diagnosis}>{intl.formatMessage(m.csDiagnosis)}</label>
              <input id={ids.diagnosis} value={diagnosis} onChange={(e) => setDiagnosis(e.target.value)} className={control} />
            </div>
            <div className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
              <input id={ids.provisional} type="checkbox" checked={provisional} onChange={(e) => setProvisional(e.target.checked)} />
              <label htmlFor={ids.provisional}>{intl.formatMessage(m.csProvisional)}</label>
            </div>
            <div>
              <button type="submit" className={button} disabled={diagnose.isPending || diagnosis.trim() === ''}>
                {intl.formatMessage(m.csAddDiagnosis)}
              </button>
            </div>
            <Outcome mutation={diagnose} done={intl.formatMessage(m.csDiagnosisAdded, { name: last.diagnosis.trim() })} />
          </>,
        )}
      </section>

      <section className={section}>
        {form(
          () => {
            setLast({ ...last, test })
            order.mutate({ what: 'orders', body: { id: newResourceId(), kind: 'service', concept: { display: test.trim() } } }, { onSuccess: () => setTest('') })
          },
          <>
            <div className={field}>
              <label htmlFor={ids.test}>{intl.formatMessage(m.csTest)}</label>
              <input id={ids.test} value={test} onChange={(e) => setTest(e.target.value)} className={control} />
            </div>
            <div>
              <button type="submit" className={button} disabled={order.isPending || test.trim() === ''}>
                {intl.formatMessage(m.csOrderTest)}
              </button>
            </div>
            <Outcome mutation={order} done={intl.formatMessage(m.csTestOrdered, { name: last.test.trim() })} />
          </>,
        )}
      </section>

      <section className={section}>
        {form(
          () => {
            setLast({ ...last, medicine })
            prescribe.mutate(
              { what: 'orders', body: { id: newResourceId(), kind: 'medication', concept: { display: medicine.trim() }, dosage: dosage.trim() } },
              {
                onSuccess: () => {
                  setMedicine('')
                  setDosage('')
                },
              },
            )
          },
          <>
            <div className={field}>
              <label htmlFor={ids.medicine}>{intl.formatMessage(m.csMedicine)}</label>
              <input id={ids.medicine} value={medicine} onChange={(e) => setMedicine(e.target.value)} className={control} />
            </div>
            <div className={field}>
              <label htmlFor={ids.dosage}>{intl.formatMessage(m.csDosage)}</label>
              <input id={ids.dosage} value={dosage} onChange={(e) => setDosage(e.target.value)} className={control} />
            </div>
            <div>
              <button type="submit" className={button} disabled={prescribe.isPending || medicine.trim() === '' || dosage.trim() === ''}>
                {intl.formatMessage(m.csPrescribe)}
              </button>
            </div>
            {prescribe.error instanceof ConsultationError && prescribe.error.warnings.length > 0 && prescribe.variables !== undefined ? (
              <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', color: 'danger.text' })}>
                <p>{intl.formatMessage(m.csWarnings)}</p>
                <ul>
                  {prescribe.error.warnings.map((w) => (
                    <li key={`${w.code}-${w.message}`}>{w.message}</li>
                  ))}
                </ul>
                <div>
                  <button
                    type="button"
                    className={button}
                    onClick={() => {
                      const command = prescribe.variables
                      if (command?.what === 'orders' && command.body.kind === 'medication') {
                        prescribe.mutate(
                          { what: 'orders', body: { ...command.body, acknowledge: true } },
                          {
                            onSuccess: () => {
                              setMedicine('')
                              setDosage('')
                            },
                          },
                        )
                      }
                    }}
                  >
                    {intl.formatMessage(m.csPrescribeAnyway)}
                  </button>
                </div>
              </div>
            ) : (
              <Outcome mutation={prescribe} done={intl.formatMessage(m.csPrescribed, { name: last.medicine.trim() })} />
            )}
          </>,
        )}
      </section>

      <section className={section}>
        {disposition.isSuccess ? (
          <>
            <p role="status">
              {disposition.variables !== undefined && 'servicePoint' in disposition.variables
                ? intl.formatMessage(m.csSent, { servicePoint: disposition.variables.servicePoint })
                : intl.formatMessage(m.csCompleted)}
            </p>
            <div>
              <button type="button" className={button} onClick={onClose}>
                {intl.formatMessage(m.csBack)}
              </button>
            </div>
          </>
        ) : (
          <>
            <div className={css({ display: 'flex', gap: '2', alignItems: 'end', flexWrap: 'wrap' })}>
              <div className={field}>
                <label htmlFor={ids.next}>{intl.formatMessage(m.csSendTo)}</label>
                <select id={ids.next} value={next} onChange={(e) => setNext(e.target.value)} className={control}>
                  {SERVICE_POINTS.filter((s) => s !== 'consultation').map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </div>
              <button type="button" className={button} disabled={disposition.isPending} onClick={() => disposition.mutate({ servicePoint: next })}>
                {intl.formatMessage(m.csSend)}
              </button>
            </div>
            <div>
              <button type="button" className={button} disabled={disposition.isPending} onClick={() => disposition.mutate({ complete: true })}>
                {intl.formatMessage(m.csComplete)}
              </button>
            </div>
            <Outcome mutation={disposition} done="" />
          </>
        )}
      </section>
    </section>
  )
}
