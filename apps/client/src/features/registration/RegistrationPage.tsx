import { DataTable, Field, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import type { Registration } from '../../platform'
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { candidates, type Candidate } from './candidates'
import { canRegister, emptyForm, toPatient, type RegistrationForm } from './patient'

const button = css({
  alignSelf: 'flex-start',
  minHeight: 'control',
  px: 'controlX',
  bg: 'accent.default',
  color: 'accent.fg',
  borderRadius: 'md',
  fontSize: 'control',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})
const grid = css({ display: 'grid', gridTemplateColumns: { base: '1fr', md: '1fr 1fr' }, gap: '4' })

/**
 * Reception registration (patient-identity.md § "Did you mean" at registration): the clerk enters
 * the patient, checks for existing records, then picks one or, having confirmed it is none of
 * them, registers a new provisional patient. A duplicate is never created without that
 * confirmation; the server enforces it too. With too little to match on, the clerk searches the
 * registry by hand first.
 */
export function RegistrationPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const [form, setForm] = useState<RegistrationForm>(emptyForm)
  const [confirmed, setConfirmed] = useState(false)
  const [chosen, setChosen] = useState<Candidate | undefined>()
  const [view, setView] = useState<DataTableView>(emptyView)
  const patient = () => toPatient(form, platform.registries.ninSystem, new Date().getFullYear())
  const check = useMutation({
    mutationFn: (confirmedNew: boolean): Promise<Registration> => platform.registries.register(patient(), confirmedNew),
  })
  const search = useMutation({
    mutationFn: () => {
      const params: Record<string, string> = {}
      if (form.family.trim() !== '') params['family'] = form.family.trim()
      if (form.given.trim() !== '') params['given'] = form.given.trim()
      const year = patient().birthDate?.slice(0, 4)
      if (year !== undefined) params['birthdate'] = year
      if (form.village.trim() !== '') params['address-city'] = form.village.trim()
      return platform.fhir.search('Patient', params, { via: 'POST' })
    },
  })
  const set = (field: keyof RegistrationForm) => (event: { currentTarget: { value: string } }) => {
    setForm({ ...form, [field]: event.currentTarget.value })
    setConfirmed(false)
    setChosen(undefined)
    check.reset()
    search.reset()
  }
  const startOver = () => {
    setForm(emptyForm)
    setConfirmed(false)
    setChosen(undefined)
    check.reset()
    search.reset()
  }
  const columns: DataTableColumn<Candidate>[] = [
    { id: 'name', header: intl.formatMessage(messages.regColName), cell: (c) => c.name, sortValue: (c) => c.name },
    { id: 'born', header: intl.formatMessage(messages.regColBirth), cell: (c) => c.birthDate, sortValue: (c) => c.birthDate },
    { id: 'sex', header: intl.formatMessage(messages.regColSex), cell: (c) => c.sex },
    { id: 'mrn', header: intl.formatMessage(messages.regColMrn), cell: (c) => c.mrn },
    { id: 'grade', header: intl.formatMessage(messages.regColMatch), cell: (c) => c.grade, sortValue: (c) => c.score },
  ]
  const result = check.data
  const listed: Candidate[] | undefined =
    result?.kind === 'candidates' ? candidates(result.bundle) : search.data === undefined ? undefined : candidates(search.data)

  if (result?.kind === 'registered') {
    const outcome =
      result.outcome === 'auto-linked' ? messages.regOutcomeLinked : result.outcome === 'queued' ? messages.regOutcomeQueued : messages.regOutcomeNew
    return (
      <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.regTitle)}</h1>
        <p role="status">
          {intl.formatMessage(messages.regRegistered)} <strong>{result.mrn}</strong>
        </p>
        <p>{intl.formatMessage(outcome)}</p>
        <Link to="/patients/$id" params={{ id: result.id }}>
          {intl.formatMessage(messages.ptOpen)}
        </Link>
        <button type="button" className={button} onClick={startOver}>
          {intl.formatMessage(messages.regAnother)}
        </button>
      </section>
    )
  }

  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.regTitle)}</h1>
      <form
        className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (canRegister(form)) check.mutate(false)
        }}
      >
        <div className={grid}>
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.regFamily)}</Field.Label>
            <Field.Input value={form.family} onChange={set('family')} autoComplete="off" />
          </Field.Root>
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.regGiven)}</Field.Label>
            <Field.Input value={form.given} onChange={set('given')} autoComplete="off" />
          </Field.Root>
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.regSex)}</Field.Label>
            <Field.Select value={form.sex} onChange={set('sex')}>
              <option value="">{intl.formatMessage(messages.regSexChoose)}</option>
              <option value="female">{intl.formatMessage(messages.regFemale)}</option>
              <option value="male">{intl.formatMessage(messages.regMale)}</option>
              <option value="other">{intl.formatMessage(messages.regOther)}</option>
              <option value="unknown">{intl.formatMessage(messages.regUnknown)}</option>
            </Field.Select>
          </Field.Root>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.regBirthDate)}</Field.Label>
            <Field.Input type="date" value={form.birthDate} onChange={set('birthDate')} />
          </Field.Root>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.regAge)}</Field.Label>
            <Field.Input inputMode="numeric" value={form.age} onChange={set('age')} disabled={form.birthDate !== ''} />
          </Field.Root>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.regNin)}</Field.Label>
            <Field.Input value={form.nin} onChange={set('nin')} autoComplete="off" />
          </Field.Root>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.regPhone)}</Field.Label>
            <Field.Input type="tel" value={form.phone} onChange={set('phone')} autoComplete="off" />
          </Field.Root>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.regDistrict)}</Field.Label>
            <Field.Input value={form.district} onChange={set('district')} />
          </Field.Root>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.regVillage)}</Field.Label>
            <Field.Input value={form.village} onChange={set('village')} />
          </Field.Root>
        </div>
        <button type="submit" className={button} disabled={!canRegister(form) || check.isPending}>
          {intl.formatMessage(messages.regFind)}
        </button>
      </form>

      {check.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.regFailed)}
        </p>
      ) : null}

      {result?.kind === 'insufficient' ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <p role="status">{intl.formatMessage(messages.regInsufficient)}</p>
          <button type="button" className={button} disabled={search.isPending} onClick={() => search.mutate()}>
            {intl.formatMessage(messages.regSearch)}
          </button>
          {search.isError ? <p role="alert">{intl.formatMessage(messages.regSearchFailed)}</p> : null}
          {listed !== undefined && listed.length === 0 ? <p role="status">{intl.formatMessage(messages.regNoResults)}</p> : null}
        </div>
      ) : null}

      {listed !== undefined && listed.length > 0 ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(messages.regCandidates)}</h2>
          <p className={css({ color: 'fg.muted' })}>{intl.formatMessage(messages.regCandidatesHint)}</p>
          <DataTable
            caption={intl.formatMessage(messages.regCandidates)}
            columns={columns}
            data={listed}
            getRowId={(c) => c.id}
            view={view}
            onViewChange={setView}
            onRowActivate={setChosen}
            height="18rem"
          />
          {chosen === undefined ? null : (
            <p role="status">
              {intl.formatMessage(messages.regChosen)} <strong>{chosen.name}</strong> {chosen.mrn}{' '}
              <Link to="/patients/$id" params={{ id: chosen.id }}>
                {intl.formatMessage(messages.ptOpen)}
              </Link>
            </p>
          )}
        </div>
      ) : null}

      {result !== undefined && chosen === undefined ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <label className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
            <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.currentTarget.checked)} />
            {intl.formatMessage(messages.regNoneOfThese)}
          </label>
          <button type="button" className={button} disabled={!confirmed || check.isPending} onClick={() => check.mutate(true)}>
            {intl.formatMessage(messages.regRegisterNew)}
          </button>
        </div>
      ) : null}
    </section>
  )
}
