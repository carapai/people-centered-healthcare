import type { Patient } from '@health/fhir-types'
import { css } from '@health/design-system/css'
import { useMutation } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { receptionMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { newResourceId } from '../../platform/ids'
import { ReceptionError, SERVICE_POINTS, checkIn, mrn, patientName, patients } from './api'
import { button, control, field } from './styles'

/**
 * Reception (#102; clinical-workspaces.md): find a registered patient, then check them in at a
 * service point. The server starts the Encounter and puts the patient in that service point's
 * queue; a patient already checked in there is refused.
 */
export function ReceptionPage() {
  const intl = useIntl()
  const ids = {
    family: useId(),
    given: useId(),
    servicePoint: useId(),
    emergency: useId(),
  }
  const platform = usePlatform()
  const [family, setFamily] = useState('')
  const [given, setGiven] = useState('')
  const [chosen, setChosen] = useState<Patient | undefined>()
  const [servicePoint, setServicePoint] = useState<string>(SERVICE_POINTS[0])
  const [emergency, setEmergency] = useState(false)

  const search = useMutation({
    mutationFn: () => {
      const params: Record<string, string> = {}
      if (family.trim() !== '') params['family'] = family.trim()
      if (given.trim() !== '') params['given'] = given.trim()
      return platform.fhir.search('Patient', params, { via: 'POST' })
    },
  })
  const admit = useMutation({
    mutationFn: (patient: Patient) => checkIn(platform.transport, newResourceId(), `Patient/${patient.id ?? ''}`, servicePoint, emergency),
  })
  const found = patients(search.data)
  const refused = (error: unknown) =>
    intl.formatMessage(m.rcFailed, {
      reason: error instanceof ReceptionError ? error.reason || String(error.status) : '',
    })

  return (
    <section
      className={css({
        display: 'flex',
        flexDirection: 'column',
        gap: '4',
        maxWidth: '3xl',
      })}
    >
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.rcTitle)}</h1>
      <form
        className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
        onSubmit={(event) => {
          event.preventDefault()
          setChosen(undefined)
          admit.reset()
          search.mutate()
        }}
      >
        <div className={field}>
          <label htmlFor={ids.family}>{intl.formatMessage(m.rcFamily)}</label>
          <input id={ids.family} value={family} onChange={(e) => setFamily(e.target.value)} className={control} />
        </div>
        <div className={field}>
          <label htmlFor={ids.given}>{intl.formatMessage(m.rcGiven)}</label>
          <input id={ids.given} value={given} onChange={(e) => setGiven(e.target.value)} className={control} />
        </div>
        <div>
          <button type="submit" className={button} disabled={search.isPending || (family.trim() === '' && given.trim() === '')}>
            {intl.formatMessage(m.rcFind)}
          </button>
        </div>
      </form>
      {search.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {refused(search.error)}
        </p>
      ) : null}
      {search.isSuccess && found.length === 0 ? <p role="status">{intl.formatMessage(m.rcNoneFound)}</p> : null}
      {chosen === undefined && found.length > 0 ? (
        <ul
          className={css({
            display: 'flex',
            flexDirection: 'column',
            gap: '2',
          })}
        >
          {found.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                className={button}
                onClick={() => {
                  setChosen(p)
                  admit.reset()
                }}
              >
                {intl.formatMessage(m.rcChoose, { name: patientName(p) })}
              </button>
              <span className={css({ ml: '2' })}>{[mrn(p), p.birthDate].filter(Boolean).join(' · ')}</span>
            </li>
          ))}
        </ul>
      ) : null}
      {chosen === undefined ? null : (
        <form
          className={css({
            display: 'flex',
            flexDirection: 'column',
            gap: '3',
          })}
          onSubmit={(event) => {
            event.preventDefault()
            admit.mutate(chosen)
          }}
        >
          <p role="status">{intl.formatMessage(m.rcChosen, { name: patientName(chosen) })}</p>
          <div className={field}>
            <label htmlFor={ids.servicePoint}>{intl.formatMessage(m.rcServicePoint)}</label>
            <select id={ids.servicePoint} value={servicePoint} onChange={(e) => setServicePoint(e.target.value)} className={control}>
              {SERVICE_POINTS.map((sp) => (
                <option key={sp} value={sp}>
                  {sp}
                </option>
              ))}
            </select>
          </div>
          <div className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
            <input id={ids.emergency} type="checkbox" checked={emergency} onChange={(e) => setEmergency(e.target.checked)} />
            <label htmlFor={ids.emergency}>{intl.formatMessage(m.rcEmergency)}</label>
          </div>
          <div>
            <button type="submit" className={button} disabled={admit.isPending || admit.isSuccess}>
              {intl.formatMessage(m.rcCheckIn)}
            </button>
          </div>
          {admit.isSuccess ? (
            <p role="status">
              {intl.formatMessage(m.rcCheckedIn, {
                name: patientName(chosen),
                servicePoint,
              })}
            </p>
          ) : null}
          {admit.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {refused(admit.error)}
            </p>
          ) : null}
        </form>
      )}
    </section>
  )
}
