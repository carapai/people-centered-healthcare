import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { placesMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { type FacilityReporting, type OrgUnitFlag, PlacesError, REPORTING, type ReportingForm, reportingQuery, setReporting } from './api'

const page = css({ display: 'flex', flexDirection: 'column', gap: '6', maxWidth: '3xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const card = css({ display: 'flex', flexDirection: 'column', gap: '3', p: '4', borderWidth: '1px', borderColor: 'border.default', borderRadius: 'md' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '3' })
const row = css({ display: 'flex', flexWrap: 'wrap', gap: '2', alignItems: 'center' })
const flagged = css({ color: 'danger.text', listStyle: 'disc', ps: '6' })
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

function reasonOf(error: unknown): string {
  if (error instanceof PlacesError) return error.detail === '' ? String(error.status) : error.detail
  return String(error)
}

/**
 * Each facility's HMIS reporting settings at central (ADR 0069; #560): its HMIS org unit ID,
 * flagged when the NHFR does not agree, and the forms (DHIS2 data sets) reported through the
 * platform. Someone whose role gives `places.manage` changes them; `reporting.view` reads them,
 * so the district knows which forms it no longer keys.
 */
export function ReportingPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const settings = useQuery(reportingQuery(transport))
  if (settings.data === undefined) {
    if (settings.isPending) return <p role="status">{intl.formatMessage(m.plLoading)}</p>
    const forbidden = settings.error instanceof PlacesError && settings.error.status === 403
    return (
      <section className={page}>
        <h1 className={heading}>{intl.formatMessage(m.plReportingTitle)}</h1>
        <p role="alert" className={css({ color: forbidden ? 'fg.muted' : 'danger.text' })}>
          {forbidden ? intl.formatMessage(m.plReportingForbidden) : intl.formatMessage(m.plFailed, { reason: reasonOf(settings.error) })}
        </p>
      </section>
    )
  }
  const { manage, metadataRecorded, forms, facilities } = settings.data
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.plReportingTitle)}</h1>
      <p>{intl.formatMessage(m.plReportingIntro)}</p>
      {manage ? null : <p>{intl.formatMessage(m.plReadOnly)}</p>}
      {metadataRecorded ? null : <p>{intl.formatMessage(m.plNoMetadata)}</p>}
      {facilities.length === 0 ? <p>{intl.formatMessage(m.plNoFacilities)}</p> : null}
      <div className={list}>
        {facilities.map((facility) =>
          manage ? <EditFacility key={facility.code} facility={facility} forms={forms} /> : <ShowFacility key={facility.code} facility={facility} forms={forms} />,
        )}
      </div>
    </section>
  )
}

/** A flag, as a person reads it. */
function useFlagText() {
  const intl = useIntl()
  return (flag: OrgUnitFlag) => {
    switch (flag.kind) {
      case 'unknownToNhfr':
        return intl.formatMessage(m.plFlag_unknownToNhfr, flag)
      case 'nhfrListsElsewhere':
        return intl.formatMessage(m.plFlag_nhfrListsElsewhere, flag)
      case 'nhfrListsAnother':
        return intl.formatMessage(m.plFlag_nhfrListsAnother, flag)
      case 'sharedWith':
        return intl.formatMessage(m.plFlag_sharedWith, flag)
    }
  }
}

/** The facility's heading, the NHFR's org unit ID and the flags on the one set. */
function Facts({ facility, id }: { facility: FacilityReporting; id: string }) {
  const intl = useIntl()
  const flagText = useFlagText()
  return (
    <>
      <h2 id={id} className={subheading}>
        {facility.name} ({facility.code})
      </h2>
      <p>{facility.nhfrOrgUnit === null ? intl.formatMessage(m.plNhfrNone) : intl.formatMessage(m.plNhfrOrgUnit, { orgUnit: facility.nhfrOrgUnit })}</p>
      {facility.flags.length === 0 ? null : (
        <ul className={flagged}>
          {facility.flags.map((flag) => (
            <li key={flagText(flag)}>{flagText(flag)}</li>
          ))}
        </ul>
      )}
    </>
  )
}

/** A facility's settings, read only. */
function ShowFacility({ facility, forms }: { facility: FacilityReporting; forms: ReportingForm[] }) {
  const intl = useIntl()
  const id = `reporting-${facility.code}`
  const names = facility.forms.map((f) => forms.find((form) => form.id === f)?.name ?? f)
  return (
    <section className={card} aria-labelledby={id}>
      <Facts facility={facility} id={id} />
      <p>{facility.orgUnit === null ? intl.formatMessage(m.plOrgUnitNotSet) : intl.formatMessage(m.plOrgUnitSet, { orgUnit: facility.orgUnit })}</p>
      <p>{names.length === 0 ? intl.formatMessage(m.plFormsNone) : intl.formatMessage(m.plFormsOn, { forms: names.join(', ') })}</p>
    </section>
  )
}

/** A facility's settings, to change. */
function EditFacility({ facility, forms }: { facility: FacilityReporting; forms: ReportingForm[] }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const [orgUnit, setOrgUnit] = useState(facility.orgUnit ?? '')
  const [chosen, setChosen] = useState<string[]>(facility.forms)
  const saving = useMutation({
    mutationFn: () => setReporting(transport, facility.code, orgUnit, chosen),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: REPORTING }),
  })
  const choose = (form: string, on: boolean) => {
    saving.reset()
    setChosen((current) => forms.map((f) => f.id).filter((f) => (f === form ? on : current.includes(f))))
  }
  const id = `reporting-${facility.code}`
  return (
    <form
      className={card}
      aria-labelledby={id}
      onSubmit={(event) => {
        event.preventDefault()
        saving.mutate()
      }}
    >
      <Facts facility={facility} id={id} />
      <Field.Root>
        <Field.Label>{intl.formatMessage(m.plOrgUnit)}</Field.Label>
        <Field.Input
          autoComplete="off"
          value={orgUnit}
          onChange={(event) => {
            saving.reset()
            setOrgUnit(event.target.value)
          }}
        />
      </Field.Root>
      {forms.length === 0 ? null : (
        <fieldset className={list}>
          <legend>{intl.formatMessage(m.plFormsLegend)}</legend>
          {forms.map((form) => (
            <label key={form.id} className={row}>
              <input type="checkbox" checked={chosen.includes(form.id)} onChange={(event) => choose(form.id, event.target.checked)} />
              {form.name}
            </label>
          ))}
        </fieldset>
      )}
      <button type="submit" className={button} disabled={saving.isPending}>
        {intl.formatMessage(m.plSaveReporting)}
      </button>
      {saving.isSuccess ? <p role="status">{intl.formatMessage(m.plSavedReporting, { name: facility.name })}</p> : null}
      {saving.isError ? (
        <div role="alert" className={css({ color: 'danger.text' })}>
          <p>{intl.formatMessage(m.plFailed, { reason: reasonOf(saving.error) })}</p>
          {saving.error instanceof PlacesError && saving.error.problems.length > 0 ? (
            <ul className={flagged}>
              {saving.error.problems.map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </form>
  )
}
