import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { productMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import type { Transport } from '../../platform/types'
import { button, control, field } from '../reception/styles'

/** A stock product, as `/api/stock/products` gives it. */
export interface StockProduct {
  id: string
  code: string
  name: string
  baseUnit: string
  packSizes: number[]
  synonyms: string[]
  active: boolean
}

const PRODUCTS = ['stock', 'products'] as const

class SaveFailed extends Error {
  constructor(readonly reason: string) {
    super(`product not saved: ${reason}`)
    this.name = 'SaveFailed'
  }
}

async function list(transport: Transport): Promise<StockProduct[]> {
  const response = await transport.request({ method: 'GET', path: '/stock/products' })
  if (response.status !== 200) throw new Error(`products: ${response.status}`)
  return response.body as StockProduct[]
}

const empty = { code: '', name: '', baseUnit: '', packSize: '', synonyms: '' }

/**
 * The product master (#128 follow-up): each stock product, its base unit and pack size, and the
 * names it is prescribed by, so the pharmacy knows which stock product a prescription is
 * dispensed as. A product's code never changes.
 */
export function ProductsPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const ids = { code: useId(), codeHint: useId(), name: useId(), unit: useId(), pack: useId(), synonyms: useId(), synonymsHint: useId() }
  const products = useQuery({ queryKey: PRODUCTS, queryFn: () => list(platform.transport) })
  const [form, setForm] = useState(empty)
  const [saved, setSaved] = useState('')
  const set = (key: keyof typeof form) => (event: { target: { value: string } }) => setForm({ ...form, [key]: event.target.value })
  const pack = Number.parseInt(form.packSize, 10)
  const ready = form.code.trim() !== '' && form.name.trim() !== '' && form.baseUnit.trim() !== '' && pack > 0
  const save = useMutation({
    mutationFn: async () => {
      const body = {
        code: form.code.trim(),
        name: form.name.trim(),
        baseUnit: form.baseUnit.trim(),
        packSizes: [pack],
        synonyms: form.synonyms.split(',').map((s) => s.trim()).filter((s) => s !== ''),
      }
      const response = await platform.transport.request({ method: 'POST', path: '/stock/products', body })
      if (response.status !== 201) throw new SaveFailed(((response.body ?? {}) as { error?: string }).error ?? String(response.status))
      return body.name
    },
    onSuccess: (name) => {
      setSaved(intl.formatMessage(m.pdSaved, { name }))
      setForm(empty)
      void queryClient.invalidateQueries({ queryKey: PRODUCTS })
    },
  })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.pdTitle)}</h1>
      <p>{intl.formatMessage(m.pdIntro)}</p>
      {products.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.pdFailed)}
        </p>
      ) : null}
      {products.data?.length === 0 ? <p>{intl.formatMessage(m.pdNone)}</p> : null}
      <ul aria-label={intl.formatMessage(m.pdTitle)} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {(products.data ?? []).map((p) => (
          <li key={p.id}>
            <span>{intl.formatMessage(m.pdListed, { name: p.name, code: p.code, unit: p.baseUnit })}</span>
            {p.synonyms.length === 0 ? null : <span> · {intl.formatMessage(m.pdSynonyms, { names: p.synonyms.join(', ') })}</span>}
          </li>
        ))}
      </ul>
      <form
        className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (ready) save.mutate()
        }}
      >
        <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(m.pdAdd)}</h2>
        <div className={field}>
          <label htmlFor={ids.code}>{intl.formatMessage(m.pdCode)}</label>
          <input id={ids.code} aria-describedby={ids.codeHint} value={form.code} onChange={set('code')} className={control} autoComplete="off" />
          <span id={ids.codeHint}>{intl.formatMessage(m.pdCodeHint)}</span>
        </div>
        <div className={field}>
          <label htmlFor={ids.name}>{intl.formatMessage(m.pdName)}</label>
          <input id={ids.name} value={form.name} onChange={set('name')} className={control} autoComplete="off" />
        </div>
        <div className={field}>
          <label htmlFor={ids.unit}>{intl.formatMessage(m.pdBaseUnit)}</label>
          <input id={ids.unit} value={form.baseUnit} onChange={set('baseUnit')} className={control} autoComplete="off" />
        </div>
        <div className={field}>
          <label htmlFor={ids.pack}>{intl.formatMessage(m.pdPackSize)}</label>
          <input id={ids.pack} inputMode="numeric" value={form.packSize} onChange={set('packSize')} className={control} />
        </div>
        <div className={field}>
          <label htmlFor={ids.synonyms}>{intl.formatMessage(m.pdSynonymsField)}</label>
          <input id={ids.synonyms} aria-describedby={ids.synonymsHint} value={form.synonyms} onChange={set('synonyms')} className={control} autoComplete="off" />
          <span id={ids.synonymsHint}>{intl.formatMessage(m.pdSynonymsHint)}</span>
        </div>
        <div>
          <button type="submit" className={button} disabled={!ready || save.isPending}>
            {intl.formatMessage(m.pdSave)}
          </button>
        </div>
        {saved === '' ? null : <p role="status">{saved}</p>}
        {save.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(m.pdSaveFailed, { reason: save.error instanceof SaveFailed ? save.error.reason : '' })}
          </p>
        ) : null}
      </form>
    </section>
  )
}
