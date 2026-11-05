import { css } from '@health/design-system/css'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { receptionMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { ReceptionError, SERVICE_POINTS, patientName, queueKey, queueQuery, step, type QueueEntry, type QueueStep } from './api'
import { button, control, field } from './styles'

/** The steps an entry can take next, with their labels. */
const steps = (e: QueueEntry): [QueueStep, typeof m.rcCall][] =>
  e.status === 'waiting'
    ? [
        ['call', m.rcCall],
        ['leave', m.rcLeft],
      ]
    : e.status === 'called'
      ? [
          ['serve', m.rcServe],
          ['leave', m.rcLeft],
        ]
      : [['finish', m.rcFinish]]

/**
 * Queues (#102; clinical-workspaces.md): who is waiting at a service point, emergencies first,
 * then by arrival. Calling, seeing and finishing a patient are queue steps the server checks; the
 * Encounter's own status never changes here.
 */
export function QueuePage() {
  const intl = useIntl()
  const id = useId()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const [servicePoint, setServicePoint] = useState<string>(SERVICE_POINTS[0])
  const entries = useQuery({
    ...queueQuery(platform.transport, servicePoint),
    refetchInterval: 15_000,
  })
  const names = useQueries({
    queries: (entries.data ?? []).map((e) => ({
      queryKey: ['fhir', 'Patient', e.patient],
      queryFn: () => platform.fhir.read('Patient', e.patient.replace(/^Patient\//, '')),
      staleTime: 300_000,
    })),
  })
  const act = useMutation({
    mutationFn: ({ entry, next }: { entry: QueueEntry; next: QueueStep }) => step(platform.transport, entry.id, next),
    onSettled: () => queryClient.invalidateQueries({ queryKey: queueKey(servicePoint) }),
  })
  const refused = (error: unknown) =>
    intl.formatMessage(m.rcFailed, {
      reason: error instanceof ReceptionError ? error.reason || String(error.status) : '',
    })
  const time = (iso: string) => intl.formatTime(new Date(iso), { timeStyle: 'short' })
  const statusText = (e: QueueEntry) => intl.formatMessage(e.status === 'waiting' ? m.rcWaiting : e.status === 'called' ? m.rcCalled : m.rcInService)

  return (
    <section
      className={css({
        display: 'flex',
        flexDirection: 'column',
        gap: '4',
        maxWidth: '4xl',
      })}
    >
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.rcQueuesTitle)}</h1>
      <div className={field}>
        <label htmlFor={id}>{intl.formatMessage(m.rcServicePoint)}</label>
        <select id={id} value={servicePoint} onChange={(e) => setServicePoint(e.target.value)} className={control}>
          {SERVICE_POINTS.map((sp) => (
            <option key={sp} value={sp}>
              {sp}
            </option>
          ))}
        </select>
      </div>
      {entries.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {refused(entries.error)}
        </p>
      ) : null}
      {act.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {refused(act.error)}
        </p>
      ) : null}
      {entries.data?.length === 0 ? <p role="status">{intl.formatMessage(m.rcQueueEmpty)}</p> : null}
      {entries.data !== undefined && entries.data.length > 0 ? (
        <ol
          aria-label={servicePoint}
          className={css({
            display: 'flex',
            flexDirection: 'column',
            gap: '3',
          })}
        >
          {entries.data.map((e, index) => {
            const name = patientName(names[index]?.data) || e.patient
            return (
              <li
                key={e.id}
                className={css({
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: '3',
                  alignItems: 'center',
                  borderBottom: '1px solid',
                  borderColor: 'border',
                  pb: '2',
                })}
              >
                <span className={css({ fontWeight: 'semibold' })}>{name}</span>
                {e.priority === 'routine' ? null : <span className={css({ color: 'danger.text' })}>{intl.formatMessage(m.rcEmergencyTag)}</span>}
                <span>{statusText(e)}</span>
                <span>{intl.formatMessage(m.rcArrived, { time: time(e.arrived) })}</span>
                {steps(e).map(([next, label]) => (
                  <button key={next} type="button" className={button} disabled={act.isPending} onClick={() => act.mutate({ entry: e, next })}>
                    {intl.formatMessage(label, { name })}
                  </button>
                ))}
              </li>
            )
          })}
        </ol>
      ) : null}
    </section>
  )
}
