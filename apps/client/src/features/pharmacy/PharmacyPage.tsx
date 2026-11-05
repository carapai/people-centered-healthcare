import { css } from '@health/design-system/css'
import { useQueries, useQuery } from '@tanstack/react-query'
import { useIntl } from 'react-intl'

import { pharmacyMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import type { Transport } from '../../platform/types'
import { patientName } from '../reception/api'

/** A prescription on the worklist, as `/api/pharmacy/worklist` gives it. */
interface Prescription {
  id: string
  patient: string
  medicine: { text: string }
  dosage: string
  days: number | null
  prescriber: string
  authored: string
}

async function worklist(transport: Transport): Promise<Prescription[]> {
  const response = await transport.request({ method: 'GET', path: '/pharmacy/worklist' })
  if (response.status !== 200) throw new Error(`pharmacy worklist: ${response.status}`)
  return response.body as Prescription[]
}

/**
 * The pharmacy worklist (#108; inventory-and-procurement.md § Dispensing): the facility's open
 * prescriptions, oldest first. A prescription has no stock effect; dispensing starts here.
 */
export function PharmacyPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const list = useQuery({ queryKey: ['pharmacy', 'worklist'], queryFn: () => worklist(platform.transport), refetchInterval: 15_000 })
  const names = useQueries({
    queries: (list.data ?? []).map((p) => ({
      queryKey: ['fhir', 'Patient', p.patient],
      queryFn: () => platform.fhir.read('Patient', p.patient.replace(/^Patient\//, '')),
      staleTime: 300_000,
    })),
  })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.phTitle)}</h1>
      <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(m.phWorklist)}</h2>
      {list.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.phFailed)}
        </p>
      ) : null}
      {list.data?.length === 0 ? <p role="status">{intl.formatMessage(m.phEmpty)}</p> : null}
      <ol aria-label={intl.formatMessage(m.phWorklist)} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        {(list.data ?? []).map((p, index) => (
          <li key={p.id} className={css({ borderBottom: '1px solid', borderColor: 'border', pb: '2' })}>
            <p className={css({ fontWeight: 'semibold' })}>{patientName(names[index]?.data) || p.patient}</p>
            <p>{intl.formatMessage(m.phPrescription, { medicine: p.medicine.text, dosage: p.dosage })}</p>
            {p.days === null ? null : <p>{intl.formatMessage(m.phDays, { days: p.days })}</p>}
            <p>{intl.formatMessage(m.phBy, { name: p.prescriber, time: intl.formatDate(new Date(p.authored), { dateStyle: 'medium', timeStyle: 'short' }) })}</p>
          </li>
        ))}
      </ol>
    </section>
  )
}
