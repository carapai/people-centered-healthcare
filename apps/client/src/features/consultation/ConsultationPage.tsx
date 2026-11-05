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
import { ConsultationError, dispose, record, summaryKey, type Command } from './api'

const section = css({ display: 'flex', flexDirection: 'column', gap: '2', borderTop: '1px solid', borderColor: 'border', pt: '3' })
const heading = css({ fontSize: 'lg', fontWeight: 'semibold' })

/** Someone in the consultation, by their queue entry. */
interface Open {
  entry: QueueEntry
  name: string
}

/**
 * The consultation (#104; clinical-workspaces.md): the patients waiting at consultation, and for
 * the one being seen, everything on one page: the shared record's summary, the note, diagnoses,
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

function Workspace({ open, onClose }: { open: Open; onClose: () => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { note: useId(), diagnosis: useId(), provisional: useId(), test: useId(), medicine: useId(), dosage: useId(), next: useId() }
  const encounter = open.entry.encounter.replace(/^Encounter\//, '')
  const patient = open.entry.patient.replace(/^Patient\//, '')
  const summary = useQuery({ queryKey: summaryKey(patient), queryFn: ({ signal }) => platform.fhir.summary(patient, { signal }) })

  const [text, setText] = useState('')
  const [diagnosis, setDiagnosis] = useState('')
  const [provisional, setProvisional] = useState(false)
  const [test, setTest] = useState('')
  const [medicine, setMedicine] = useState('')
  const [dosage, setDosage] = useState('')
  const [next, setNext] = useState<string>(SERVICE_POINTS.find((s) => s !== 'consultation') ?? 'pharmacy')
  const [last, setLast] = useState({ diagnosis: '', test: '', medicine: '' })

  const note = useCommand(encounter, patient)
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

      <section className={section}>
        {form(
          () => note.mutate({ what: 'notes', body: { id: newResourceId(), text } }, { onSuccess: () => setText('') }),
          <>
            <div className={field}>
              <label htmlFor={ids.note}>{intl.formatMessage(m.csNote)}</label>
              <textarea id={ids.note} rows={4} value={text} onChange={(e) => setText(e.target.value)} className={control} />
            </div>
            <div>
              <button type="submit" className={button} disabled={note.isPending || text.trim() === ''}>
                {intl.formatMessage(m.csSaveNote)}
              </button>
            </div>
            <Outcome mutation={note} done={intl.formatMessage(m.csNoteSaved)} />
          </>,
        )}
      </section>

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
            <Outcome mutation={prescribe} done={intl.formatMessage(m.csPrescribed, { name: last.medicine.trim() })} />
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
