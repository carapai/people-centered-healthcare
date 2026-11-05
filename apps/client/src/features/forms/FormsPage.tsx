import type { Questionnaire, QuestionnaireResponse } from '@health/fhir-types'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { formMessages } from '../../i18n/messages'
import { FhirError, fhirKeys, newResourceId, searchQuery, usePlatform } from '../../platform'
import { canonical, resources } from './api'
import { initialResponse } from './model'
import { QuestionnaireRenderer } from './QuestionnaireRenderer'

const button = css({
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

/** A response being filled, and the version of it the server last stored. */
interface Open {
  questionnaire: Questionnaire
  response: QuestionnaireResponse
  versionId: string | undefined
  /** Renders the renderer afresh when a different response is opened. */
  key: string
}

/**
 * Forms (client-application.md § Forms; #92): pick a Questionnaire, start a response or continue
 * a saved draft, and fill it in the QuestionnaireRenderer. Drafts are saved on the server, not
 * in the browser (ADR 0037), and the server validates every save and submission.
 */
export function FormsPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const pickId = useId()
  const questionnaires = useQuery(searchQuery(platform.fhir, 'Questionnaire', { status: 'active' }))
  const [chosen, setChosen] = useState('')
  const [open, setOpen] = useState<Open | undefined>()
  const [notice, setNotice] = useState<'saved' | 'submitted' | undefined>()
  const list = questionnaires.data === undefined ? [] : resources<Questionnaire>(questionnaires.data)
  const picked = list.find((q) => canonical(q) === chosen)
  // Drafts are searched by status and kept to the chosen form here: central searches no
  // canonical references until activation brings their SearchParameters (#89).
  const draftParams = { status: 'in-progress' }
  const drafts = useQuery({ ...searchQuery(platform.fhir, 'QuestionnaireResponse', draftParams), enabled: picked !== undefined })

  const mine = drafts.data === undefined ? [] : resources<QuestionnaireResponse>(drafts.data).filter((draft) => draft.questionnaire === chosen)

  const save = useMutation({
    mutationFn: ({ response }: { response: QuestionnaireResponse; submitted: boolean }) =>
      platform.forms.saveResponse(response, open?.versionId),
    onSuccess: (stored, { submitted }) => {
      setNotice(submitted ? 'submitted' : 'saved')
      setOpen(submitted || open === undefined ? undefined : { ...open, response: stored, versionId: stored.meta?.versionId })
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: fhirKeys.search('QuestionnaireResponse', draftParams) }),
  })

  const start = (questionnaire: Questionnaire, response?: QuestionnaireResponse) => {
    const draft = response ?? { ...initialResponse(questionnaire), id: newResourceId() }
    setNotice(undefined)
    save.reset()
    setOpen({ questionnaire, response: draft, versionId: response?.meta?.versionId, key: `${draft.id ?? ''}-${response?.meta?.versionId ?? 'new'}` })
  }
  const time = (iso: string | undefined) => (iso === undefined ? '' : intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' }))

  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(formMessages.fmTitle)}</h1>
      {questionnaires.data === undefined ? (
        questionnaires.isPending ? (
          <p role="status">{intl.formatMessage(formMessages.fmLoading)}</p>
        ) : (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(formMessages.fmFailed)}
          </p>
        )
      ) : list.length === 0 ? (
        <p role="status">{intl.formatMessage(formMessages.fmNone)}</p>
      ) : (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', maxWidth: 'md' })}>
          <label htmlFor={pickId}>{intl.formatMessage(formMessages.fmChooseForm)}</label>
          <select
            id={pickId}
            value={chosen}
            onChange={(event) => {
              setChosen(event.target.value)
              setOpen(undefined)
            }}
            className={css({ minHeight: 'control', px: '2', borderRadius: 'md', border: '1px solid', borderColor: 'border' })}
          >
            <option value="">{intl.formatMessage(formMessages.fmChoose)}</option>
            {list.map((q) => (
              <option key={canonical(q)} value={canonical(q)}>
                {q.title ?? q.name ?? canonical(q)}
              </option>
            ))}
          </select>
        </div>
      )}

      {notice === undefined ? null : (
        <p role="status">{intl.formatMessage(notice === 'saved' ? formMessages.fmSaved : formMessages.fmSubmitted)}</p>
      )}
      {save.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(save.error instanceof FhirError && save.error.status === 412 ? formMessages.fmConflict : formMessages.fmSaveFailed)}
        </p>
      ) : null}

      {picked === undefined || open !== undefined ? null : (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <div>
            <button type="button" className={button} onClick={() => start(picked)}>
              {intl.formatMessage(formMessages.fmOpen)}
            </button>
          </div>
          {mine.length === 0 ? null : (
            <section aria-labelledby={`${pickId}-drafts`} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
              <h2 id={`${pickId}-drafts`} className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
                {intl.formatMessage(formMessages.fmDrafts)}
              </h2>
              <ul className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
                {mine.map((draft) => (
                  <li key={draft.id}>
                    <button type="button" className={button} onClick={() => start(picked, draft)}>
                      {intl.formatMessage(formMessages.fmContinue, { saved: time(draft.meta?.lastUpdated) })}
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>
      )}

      {open === undefined ? null : (
        <section aria-label={open.questionnaire.title ?? canonical(open.questionnaire)}>
          <h2 className={css({ fontSize: 'xl', fontWeight: 'semibold', mb: '3' })}>{open.questionnaire.title ?? canonical(open.questionnaire)}</h2>
          <QuestionnaireRenderer
            key={open.key}
            questionnaire={open.questionnaire}
            initial={open.response}
            busy={save.isPending}
            onSave={(response) => save.mutate({ response: { ...response, status: 'in-progress' }, submitted: false })}
            onSubmit={(response) => save.mutate({ response, submitted: true })}
          />
        </section>
      )}
    </section>
  )
}
