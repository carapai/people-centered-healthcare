import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import type { Portal } from '../../platform/types'
import { REGISTRATION, canSubmit, registrationQuery, submitRegistration, type Answers, type Registration } from './api'

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

const empty: Answers = { given: '', family: '', birthDate: '', gender: '' }

/** What the patient is told about their registration. */
function statusMessage(registration: Registration) {
  if (registration.status === 'completed') {
    return registration.access === 'records' ? messages.portalLinked : messages.portalRegistered
  }
  return messages.portalInReview
}

/**
 * The patient portal (patient-portal.md; #111): a patient signed in with their phone registers
 * themselves, then sees where their registration is. The server decides everything; below
 * level3 the patient sees only what they submitted.
 */
export function PortalPage({ portal }: { portal: Portal }) {
  const intl = useIntl()
  const queryClient = useQueryClient()
  const signedIn = portal.session.accessToken() !== undefined
  const registration = useQuery({ ...registrationQuery(portal.transport), enabled: signedIn })
  const [answers, setAnswers] = useState<Answers>(empty)
  const submit = useMutation({
    mutationFn: () => submitRegistration(portal.transport, answers),
    onSuccess: (result) => queryClient.setQueryData(REGISTRATION, result),
  })
  const set = (field: keyof Answers) => (event: { target: { value: string } }) => setAnswers({ ...answers, [field]: event.target.value })

  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '2xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.portalTitle)}</h1>
      {!signedIn ? (
        <>
          <p>{intl.formatMessage(messages.portalIntro)}</p>
          <button type="button" className={button} onClick={() => void portal.signIn.start('/portal')}>
            {intl.formatMessage(messages.portalSignIn)}
          </button>
        </>
      ) : registration.isPending ? (
        <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
      ) : registration.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.portalFailed)}
        </p>
      ) : registration.data !== null ? (
        <>
          <p role="status">{intl.formatMessage(statusMessage(registration.data))}</p>
          {registration.data.status === 'completed' ? null : (
            <button type="button" className={button} disabled={registration.isFetching} onClick={() => void registration.refetch()}>
              {intl.formatMessage(messages.portalCheckAgain)}
            </button>
          )}
        </>
      ) : (
        <form
          className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}
          onSubmit={(event) => {
            event.preventDefault()
            if (canSubmit(answers)) submit.mutate()
          }}
        >
          <p>{intl.formatMessage(messages.portalRegisterHint)}</p>
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.regGiven)}</Field.Label>
            <Field.Input value={answers.given} onChange={set('given')} autoComplete="given-name" />
          </Field.Root>
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.regFamily)}</Field.Label>
            <Field.Input value={answers.family} onChange={set('family')} autoComplete="family-name" />
          </Field.Root>
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.regBirthDate)}</Field.Label>
            <Field.Input type="date" value={answers.birthDate} onChange={set('birthDate')} autoComplete="bday" />
          </Field.Root>
          <Field.Root required>
            <Field.Label>{intl.formatMessage(messages.regSex)}</Field.Label>
            <Field.Select value={answers.gender} onChange={set('gender')}>
              <option value="">{intl.formatMessage(messages.regSexChoose)}</option>
              <option value="female">{intl.formatMessage(messages.regFemale)}</option>
              <option value="male">{intl.formatMessage(messages.regMale)}</option>
              <option value="other">{intl.formatMessage(messages.regOther)}</option>
              <option value="unknown">{intl.formatMessage(messages.regUnknown)}</option>
            </Field.Select>
          </Field.Root>
          <button type="submit" className={button} disabled={!canSubmit(answers) || submit.isPending}>
            {intl.formatMessage(messages.portalSubmit)}
          </button>
          {submit.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {intl.formatMessage(messages.portalSubmitFailed)}
            </p>
          ) : null}
        </form>
      )}
    </section>
  )
}
