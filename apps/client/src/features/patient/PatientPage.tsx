import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { CaregiversPanel } from '../caregivers/CaregiversPanel'

/**
 * A minimal patient view for staff (#113): who the patient is, and their caregivers. The full
 * patient record screens come with the clinical slice.
 */
export function PatientPage({ id }: { id: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const patient = useQuery({
    queryKey: ['patient', id],
    queryFn: ({ signal }) => platform.fhir.read('Patient', id, { signal }),
  })
  if (patient.isError) {
    return (
      <p role="alert" className={css({ color: 'danger.text' })}>
        {intl.formatMessage(messages.ptFailed)}
      </p>
    )
  }
  if (patient.data === undefined) return <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
  const name = [patient.data.name?.[0]?.given?.join(' '), patient.data.name?.[0]?.family].filter(Boolean).join(' ')
  const mrn = patient.data.identifier?.find((i) => i.type?.coding?.some((c) => c.code === 'MR'))?.value
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '6', maxWidth: '4xl' })}>
      <div>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{name}</h1>
        <p>
          {patient.data.birthDate ?? ''} {mrn === undefined ? '' : `· ${mrn}`}
        </p>
      </div>
      <CaregiversPanel patient={id} />
    </section>
  )
}
