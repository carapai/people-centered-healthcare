import type { Questionnaire, QuestionnaireResponse } from '@health/fhir-types'
import { css } from '@health/design-system/css'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { triageMessages as m } from '../../i18n/messages'
import { newResourceId, searchQuery, usePlatform } from '../../platform'
import { dispose } from '../consultation/api'
import { resources } from '../forms/api'
import { initialResponse } from '../forms/model'
import { QuestionnaireRenderer } from '../forms/QuestionnaireRenderer'
import { patientName, queueKey, queueQuery, step, type QueueEntry } from '../reception/api'
import { button } from '../reception/styles'

/** The OPD triage form (`ug.moh.opd`, #103): vital signs, extracted as Observations, and a triage category. */
export const TRIAGE_FORM = 'https://health.go.ug/fhir/Questionnaire/opd-triage'

const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })

/** Someone being triaged, by their queue entry. */
interface Open {
  entry: QueueEntry
  name: string
}

/** A version's numbers, `MAJOR.MINOR.PATCH`. */
const parts = (q: Questionnaire) => (q.version ?? '0').split('.').map((n) => Number.parseInt(n, 10) || 0)

/** Whether `a`'s version is newer than `b`'s. */
function newer(a: Questionnaire, b: Questionnaire): boolean {
  const [x, y] = [parts(a), parts(b)]
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0)
    if (d !== 0) return d > 0
  }
  return false
}

/** The newest version of the triage form among `found`. */
function newest(found: Questionnaire[]): Questionnaire | undefined {
  return found.reduce<Questionnaire | undefined>((best, q) => (best === undefined || newer(q, best) ? q : best), undefined)
}

/**
 * Triage (#103; patient-flow.md): the patients waiting at triage and, for the one being seen, the
 * active OPD triage form for their visit. On submission the node extracts each vital sign as an
 * Observation, with the response's Provenance; the patient then waits for consultation.
 */
export function TriagePage() {
  const [open, setOpen] = useState<Open | undefined>()
  const [done, setDone] = useState<string | undefined>()
  return open === undefined ? (
    <Waiting
      done={done}
      onOpen={(next) => {
        setDone(undefined)
        setOpen(next)
      }}
    />
  ) : (
    <Form
      open={open}
      onClose={(triaged) => {
        setDone(triaged ? open.name : undefined)
        setOpen(undefined)
      }}
    />
  )
}

function Waiting({ done, onOpen }: { done: string | undefined; onOpen: (open: Open) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const entries = useQuery({ ...queueQuery(platform.transport, 'triage'), refetchInterval: 15_000 })
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
      <h1 className={heading}>{intl.formatMessage(m.trTitle)}</h1>
      {done === undefined ? null : <p role="status">{intl.formatMessage(m.trDone, { name: done })}</p>}
      <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(m.trWaiting)}</h2>
      {see.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.trFailed, { reason: '' })}
        </p>
      ) : null}
      {entries.data?.length === 0 ? <p>{intl.formatMessage(m.trNoneWaiting)}</p> : null}
      <ol aria-label={intl.formatMessage(m.trWaiting)} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {(entries.data ?? []).map((entry, index) => {
          const name = patientName(names[index]?.data) || entry.patient
          return (
            <li key={entry.id}>
              <button type="button" className={button} disabled={see.isPending} onClick={() => see.mutate(entry, { onSuccess: () => onOpen({ entry, name }) })}>
                {intl.formatMessage(m.trTriage, { name })}
              </button>
            </li>
          )
        })}
      </ol>
    </section>
  )
}

function Form({ open, onClose }: { open: Open; onClose: (triaged: boolean) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const forms = useQuery(searchQuery(platform.fhir, 'Questionnaire', { url: TRIAGE_FORM, status: 'active' }))
  const questionnaire = forms.data === undefined ? undefined : newest(resources<Questionnaire>(forms.data))
  const [initial] = useState(() => newResourceId())
  const save = useMutation({
    mutationFn: async (response: QuestionnaireResponse) => {
      // The node extracts the vital signs as it stores the response, in one transaction.
      await platform.forms.saveResponse(response, undefined)
      await dispose(platform.transport, open.entry.encounter, { servicePoint: 'consultation' })
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queueKey('triage') })
      onClose(true)
    },
  })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={heading}>{intl.formatMessage(m.trWith, { name: open.name })}</h1>
      <div>
        <button type="button" className={button} onClick={() => onClose(false)}>
          {intl.formatMessage(m.trBack)}
        </button>
      </div>
      {save.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.trFailed, { reason: save.error instanceof Error ? save.error.message : '' })}
        </p>
      ) : null}
      {questionnaire === undefined ? (
        <p role="status">{intl.formatMessage(forms.isPending ? m.trLoading : m.trNoForm)}</p>
      ) : (
        <QuestionnaireRenderer
          questionnaire={questionnaire}
          initial={{
            ...initialResponse(questionnaire),
            id: initial,
            subject: { reference: open.entry.patient },
            encounter: { reference: `Encounter/${open.entry.encounter}` },
          }}
          busy={save.isPending}
          onSubmit={(response) => save.mutate({ ...response, authored: new Date().toISOString() })}
        />
      )}
    </section>
  )
}
