import { css } from '@health/design-system/css'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { placesMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { type ImportPlan, PlacesError, type Place, applyImport, changedFields, planIsEmpty, previewImport } from './api'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '3xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const section = css({ display: 'flex', flexDirection: 'column', gap: '2' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '1', listStyle: 'disc', ps: '6' })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1' })
const button = css({
  minHeight: 'control',
  px: 'controlX',
  bg: 'accent.default',
  color: 'accent.fg',
  borderRadius: 'md',
  fontSize: 'control',
  cursor: 'pointer',
  alignSelf: 'flex-start',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})

/**
 * The NHFR import at central (ADR 0094; #873), for someone whose role gives `places.manage`:
 * choose the NHFR facility export, review what it would add, change and close, then import it.
 * Nothing is written until *Import*; a changed file must be reviewed again.
 */
export function ImportPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const [csv, setCsv] = useState<string | null>(null)
  const preview = useMutation({ mutationFn: (text: string) => previewImport(transport, text) })
  const apply = useMutation({
    mutationFn: (text: string) => applyImport(transport, text),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['places'] }),
  })
  const choose = async (file: File | undefined) => {
    preview.reset()
    apply.reset()
    setCsv(file === undefined ? null : await file.text())
  }
  const error = preview.error ?? apply.error
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.plImportTitle)}</h1>
      <p>{intl.formatMessage(m.plImportIntro)}</p>
      <label className={label}>
        {intl.formatMessage(m.plFile)}
        <input type="file" accept=".csv,text/csv" onChange={(event) => void choose(event.target.files?.[0])} />
      </label>
      <button type="button" className={button} disabled={csv === null || preview.isPending} onClick={() => {
          if (csv === null) return
          apply.reset()
          preview.mutate(csv)
        }}>
        {intl.formatMessage(m.plPreview)}
      </button>
      {error === null ? null : <Refusal error={error} />}
      {preview.data === undefined || apply.isSuccess ? null : <Review plan={preview.data} />}
      {preview.data === undefined || apply.isSuccess || planIsEmpty(preview.data) ? null : (
        <button type="button" className={button} disabled={csv === null || apply.isPending} onClick={() => csv !== null && apply.mutate(csv)}>
          {intl.formatMessage(m.plApply)}
        </button>
      )}
      {apply.data === undefined ? null : <p role="status">{intl.formatMessage(m.plApplied, { count: apply.data.written })}</p>}
    </section>
  )
}

function Refusal({ error }: { error: Error }) {
  const intl = useIntl()
  if (error instanceof PlacesError && error.status === 403) {
    return (
      <p role="alert" className={css({ color: 'fg.muted' })}>
        {intl.formatMessage(m.plImportForbidden)}
      </p>
    )
  }
  if (error instanceof PlacesError && error.problems.length > 0) {
    return (
      <div role="alert" className={css({ color: 'danger.text' })}>
        <p>{intl.formatMessage(m.plRefused)}</p>
        <ul className={list}>
          {error.problems.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      </div>
    )
  }
  return (
    <p role="alert" className={css({ color: 'danger.text' })}>
      {intl.formatMessage(m.plFailed, { reason: error instanceof PlacesError && error.detail !== '' ? error.detail : error.message })}
    </p>
  )
}

/** A place in the review: its name and code. */
function place(p: Place): string {
  return `${p.name} (${p.code})`
}

/** What the import would do, by kind. */
function Review({ plan }: { plan: ImportPlan }) {
  const intl = useIntl()
  if (planIsEmpty(plan)) {
    return (
      <section className={section} aria-label={intl.formatMessage(m.plPreview)}>
        <p role="status">{intl.formatMessage(m.plNothing)}</p>
        <p>{intl.formatMessage(m.plUnchanged, { count: plan.unchanged })}</p>
      </section>
    )
  }
  const fields = (change: ImportPlan['changed'][number]) =>
    changedFields(change)
      .map((f) => intl.formatMessage(m[`plField_${f}`]))
      .join(', ')
  return (
    <section className={section} aria-label={intl.formatMessage(m.plPreview)}>
      <Kind title={intl.formatMessage(m.plHierarchy, { count: plan.hierarchy.length })} items={plan.hierarchy.map(place)} />
      <Kind title={intl.formatMessage(m.plNew, { count: plan.new.length })} items={plan.new.map(place)} />
      <Kind title={intl.formatMessage(m.plChanged, { count: plan.changed.length })} items={plan.changed.map((c) => intl.formatMessage(m.plChange, { name: c.after.name, code: c.after.code, fields: fields(c) }))} />
      <Kind title={intl.formatMessage(m.plClosed, { count: plan.closed.length })} note={intl.formatMessage(m.plClosedNote)} items={plan.closed.map((c) => place(c.after))} />
      <Kind title={intl.formatMessage(m.plAbsent, { count: plan.absent.length })} note={intl.formatMessage(m.plAbsentNote)} items={plan.absent} />
      <p>{intl.formatMessage(m.plUnchanged, { count: plan.unchanged })}</p>
    </section>
  )
}

function Kind({ title, note, items }: { title: string; note?: string; items: string[] }) {
  if (items.length === 0) return null
  return (
    <section className={section}>
      <h2 className={subheading}>{title}</h2>
      {note === undefined ? null : <p>{note}</p>}
      <ul className={list}>
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </section>
  )
}
