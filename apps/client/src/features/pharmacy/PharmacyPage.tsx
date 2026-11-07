import { css } from '@health/design-system/css'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { pharmacyMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import type { Transport } from '../../platform/types'
import { newResourceId } from '../../platform/ids'
import { patientName } from '../reception/api'
import { button, control, field } from '../reception/styles'
import { DEFAULT_STORE } from '../stock/StockPage'
import { DispenseFailed, cancel, handOver, prepare, type Reserved } from './api'

/** A prescription on the worklist, as `/api/pharmacy/worklist` gives it. */
interface Prescription {
  id: string
  patient: string
  medicine: { text: string; code?: string | null }
  /** The stock product it resolves to in the product master. */
  product?: { kind: 'by-code' | 'by-name'; product: string } | { kind: 'ambiguous'; products: string[] } | { kind: 'unknown' }
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

const failure = (error: unknown) => (error instanceof DispenseFailed ? error.reason || String(error.status) : '')

/**
 * Dispensing one prescription (#128; ADR 0035): preparing reserves stock by first expiry and
 * moves nothing; handing over moves it; cancelling frees it.
 */
function Dispense({ prescription, name }: { prescription: Prescription; name: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { store: useId(), product: useId(), quantity: useId() }
  const resolved = prescription.product !== undefined && 'product' in prescription.product ? prescription.product.product : undefined
  const [form, setForm] = useState({ store: DEFAULT_STORE, product: resolved ?? prescription.medicine.code ?? '', quantity: '' })
  const [dispense, setDispense] = useState<{ id: string; held: Reserved[] } | undefined>()
  const [done, setDone] = useState('')
  const set = (key: keyof typeof form) => (event: { target: { value: string } }) => setForm({ ...form, [key]: event.target.value })
  const quantity = Number.parseInt(form.quantity, 10)
  const preparing = useMutation({
    mutationFn: async () => {
      const id = newResourceId()
      return { id, held: await prepare(platform.transport, prescription.id, id, { store: form.store.trim(), product: form.product.trim(), quantity }) }
    },
    onSuccess: (prepared) => {
      setDispense(prepared)
      setDone(intl.formatMessage(m.phPrepared))
    },
  })
  const finishing = useMutation({
    mutationFn: async (step: 'hand-over' | 'cancel') => {
      if (dispense === undefined) return step
      await (step === 'hand-over' ? handOver(platform.transport, dispense.id) : cancel(platform.transport, dispense.id))
      return step
    },
    onSuccess: (step) => {
      setDispense(undefined)
      setDone(intl.formatMessage(step === 'hand-over' ? m.phHandedOver : m.phCancelled))
      void queryClient.invalidateQueries({ queryKey: ['stock'] })
    },
  })
  const error = preparing.error ?? finishing.error
  const busy = preparing.isPending || finishing.isPending
  return (
    <section aria-label={intl.formatMessage(m.phDispense, { medicine: prescription.medicine.text, name })} className={css({ display: 'flex', flexDirection: 'column', gap: '2', mt: '2' })}>
      {dispense === undefined ? (
        <>
          <div className={field}>
            <label htmlFor={ids.store}>{intl.formatMessage(m.phStore)}</label>
            <input id={ids.store} value={form.store} onChange={set('store')} className={control} />
          </div>
          <div className={field}>
            <label htmlFor={ids.product}>{intl.formatMessage(m.phProduct)}</label>
            <input id={ids.product} value={form.product} onChange={set('product')} className={control} />
          </div>
          <div className={field}>
            <label htmlFor={ids.quantity}>{intl.formatMessage(m.phQuantity)}</label>
            <input id={ids.quantity} inputMode="numeric" value={form.quantity} onChange={set('quantity')} className={control} />
          </div>
          <div>
            <button
              type="button"
              className={button}
              disabled={busy || form.product.trim() === '' || form.store.trim() === '' || !(quantity > 0)}
              onClick={() => preparing.mutate()}
            >
              {intl.formatMessage(m.phPrepare)}
            </button>
          </div>
        </>
      ) : (
        <>
          <ul>
            {dispense.held.map((r) => (
              <li key={r.id}>{intl.formatMessage(m.phReserved, { batch: r.batch, expiry: r.expiry, quantity: r.quantity })}</li>
            ))}
          </ul>
          <div className={css({ display: 'flex', gap: '2' })}>
            <button type="button" className={button} disabled={busy} onClick={() => finishing.mutate('hand-over')}>
              {intl.formatMessage(m.phHandOver)}
            </button>
            <button type="button" className={button} disabled={busy} onClick={() => finishing.mutate('cancel')}>
              {intl.formatMessage(m.phCancel)}
            </button>
          </div>
        </>
      )}
      {done === '' ? null : <p role="status">{done}</p>}
      {error === null ? null : (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.phDispenseFailed, { reason: failure(error) })}
        </p>
      )}
    </section>
  )
}

/**
 * The pharmacy worklist (#108; inventory-and-procurement.md § Dispensing): the facility's open
 * prescriptions, oldest first. A prescription has no stock effect; dispensing starts here, and
 * stock moves only when a dispense is handed over (#128).
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
            <Dispense prescription={p} name={patientName(names[index]?.data) || p.patient} />
          </li>
        ))}
      </ol>
    </section>
  )
}
