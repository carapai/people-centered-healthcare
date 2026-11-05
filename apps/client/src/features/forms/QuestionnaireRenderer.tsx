import type { Questionnaire, QuestionnaireItem, QuestionnaireResponse, QuestionnaireResponseItem, QuestionnaireResponseItemAnswer } from '@health/fhir-types'
import { css } from '@health/design-system/css'
import { useForm } from '@tanstack/react-form'
import { LosslessNumber } from 'lossless-json'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { formMessages } from '../../i18n/messages'
import { type FormLayout, sections, widget } from './layout'
import { addRepeat, calculated, initialResponse, isEnabled, itemsByLinkId, missing, removeRepeat, settled, violations, withAnswers } from './model'

const control = css({ minHeight: 'control', px: '2', borderRadius: 'md', border: '1px solid', borderColor: 'border' })
const field = css({ display: 'flex', flexDirection: 'column', gap: '1', maxWidth: 'md' })
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
const quiet = css({ color: 'fg.muted', fontSize: 'sm' })
const danger = css({ color: 'danger.text' })

const NUMBER = /^-?\d+(\.\d+)?$/
const INTEGER = /^-?\d+$/

export interface QuestionnaireRendererProps {
  questionnaire: Questionnaire
  /** Its Form Layout, if the configuration has one for this `url|version`. */
  layout?: FormLayout
  /** A draft to continue; a new response otherwise. */
  initial?: QuestionnaireResponse
  /** Saves the settled response as a draft (`in-progress`). */
  onSave?: (response: QuestionnaireResponse) => void
  /** Submits the settled response (`completed`). The server re-validates it (ADR 0038). */
  onSubmit: (response: QuestionnaireResponse) => void
  /** Whether a save or submit is under way. */
  busy?: boolean
}

/**
 * The shared Questionnaire renderer (client-application.md § Forms; ADR 0038), on TanStack Form.
 * It renders a Questionnaire with its Form Layout and evaluates live behaviour with fhirpath.js
 * in lossless precise mode: enableWhen and enableWhenExpression, calculatedExpression and answer
 * constraints. Disabled questions are hidden and keep no answer; repeats keep stable ids. The
 * screen enforces nothing: the server re-validates every submission.
 */
export function QuestionnaireRenderer({ questionnaire, layout, initial, onSave, onSubmit, busy = false }: QuestionnaireRendererProps) {
  const intl = useIntl()
  const [tried, setTried] = useState(false)
  // The response is held opaque: TanStack Form's deep key types cannot traverse FHIR's (TS2589),
  // and the model functions keep it typed.
  const form = useForm({
    defaultValues: { response: (initial ?? initialResponse(questionnaire)) as unknown },
    onSubmit: ({ value }) => {
      const response = settled(questionnaire, value.response as QuestionnaireResponse)
      const blocking = violations(questionnaire, response).filter((v) => v.severity === 'error')
      if (missing(questionnaire, response).length > 0 || blocking.length > 0) return
      onSubmit({ ...response, status: 'completed' })
    },
  })
  const update = (next: QuestionnaireResponse) => form.setFieldValue('response', next)

  return (
    <form.Subscribe selector={(state) => state.values.response as QuestionnaireResponse}>
      {(response) => {
        const unanswered = missing(questionnaire, response)
        const failing = violations(questionnaire, response)
        const shown = { response, update, layout, tried, unanswered, failing }
        return (
          <form
            noValidate
            className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}
            onSubmit={(event) => {
              event.preventDefault()
              setTried(true)
              void form.handleSubmit()
            }}
          >
            {sections(questionnaire, layout).map((section, index) => (
              <section key={section.title ?? `rest-${index}`} aria-label={section.title} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
                {section.title === undefined ? null : <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{section.title}</h2>}
                {section.items.map((item) => (
                  <Items key={item.linkId} definition={item} instances={(response.item ?? []).filter((r) => r.linkId === item.linkId)} questionnaire={questionnaire} {...shown} />
                ))}
              </section>
            ))}
            {failing
              .filter((v) => v.linkId === undefined)
              .map((v) => (
                <p key={v.key} role={v.severity === 'error' ? 'alert' : 'status'} className={v.severity === 'error' ? danger : quiet}>
                  {v.human}
                </p>
              ))}
            {tried && unanswered.length > 0 ? (
              <p role="alert" className={danger}>
                {intl.formatMessage(formMessages.fmMissing, { count: unanswered.length })}
              </p>
            ) : null}
            <div className={css({ display: 'flex', gap: '2' })}>
              {onSave === undefined ? null : (
                <button type="button" className={button} disabled={busy} onClick={() => onSave(settled(questionnaire, response))}>
                  {intl.formatMessage(formMessages.fmSave)}
                </button>
              )}
              <button type="submit" className={button} disabled={busy}>
                {intl.formatMessage(formMessages.fmSubmit)}
              </button>
            </div>
          </form>
        )
      }}
    </form.Subscribe>
  )
}

interface Shown {
  questionnaire: Questionnaire
  response: QuestionnaireResponse
  update: (next: QuestionnaireResponse) => void
  layout: FormLayout | undefined
  tried: boolean
  unanswered: string[]
  failing: ReturnType<typeof violations>
}

/** Every instance of one Questionnaire item: a repeating group has one or more. */
function Items({ definition, instances, ...shown }: Shown & { definition: QuestionnaireItem; instances: QuestionnaireResponseItem[] }) {
  const intl = useIntl()
  if (!isEnabled(definition, shown.response)) return null
  if (definition.type === 'display') return <p className={quiet}>{definition.text}</p>
  const label = definition.text ?? definition.linkId
  return (
    <>
      {instances.map((instance) =>
        definition.type === 'group' ? (
          <fieldset key={instance.id} className={css({ display: 'flex', flexDirection: 'column', gap: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '3' })}>
            <legend>{label}</legend>
            {(definition.item ?? []).map((child) => (
              <Items key={child.linkId} definition={child} instances={(instance.item ?? []).filter((r) => r.linkId === child.linkId)} {...shown} />
            ))}
            {definition.repeats === true && instances.length > 1 ? (
              <div>
                <button type="button" className={button} onClick={() => shown.update(removeRepeat(shown.response, instance.id ?? ''))}>
                  {intl.formatMessage(formMessages.fmRemove, { item: label })}
                </button>
              </div>
            ) : null}
          </fieldset>
        ) : (
          <Question key={instance.id} definition={definition} instance={instance} {...shown} />
        ),
      )}
      {definition.type === 'group' && definition.repeats === true ? (
        <div>
          <button type="button" className={button} onClick={() => shown.update(addRepeat(shown.questionnaire, shown.response, definition.linkId))}>
            {intl.formatMessage(formMessages.fmAdd, { item: label })}
          </button>
        </div>
      ) : null}
    </>
  )
}

function answerText(answer: QuestionnaireResponseItemAnswer | undefined): string {
  const a = (answer ?? {}) as Record<string, unknown>
  for (const key of ['valueDecimal', 'valueInteger', 'valueDate', 'valueDateTime', 'valueString']) {
    if (a[key] !== undefined) return String(a[key])
  }
  return ''
}

/** One question. */
function Question({ definition, instance, ...shown }: Shown & { definition: QuestionnaireItem; instance: QuestionnaireResponseItem }) {
  const intl = useIntl()
  const id = useId()
  const [text, setText] = useState<string | undefined>()
  const label = definition.text ?? definition.linkId
  const hint = shown.layout?.items?.[definition.linkId]?.hint
  const computed = calculated(definition, shown.response)
  const answer = computed ?? instance.answer?.[0]
  const set = (answers: QuestionnaireResponseItemAnswer[]) => shown.update(withAnswers(shown.response, instance.id ?? '', answers))
  const required = definition.required === true
  const errors = shown.failing.filter((v) => v.linkId === definition.linkId)
  const unanswered = shown.tried && shown.unanswered.includes(definition.linkId)
  const help = `${id}-help`
  const described = hint !== undefined || errors.length > 0 || unanswered || computed !== undefined ? help : undefined
  const readOnly = definition.readOnly === true || computed !== undefined
  const chosen = widget(definition, shown.layout)
  const common = { id, 'aria-describedby': described, 'aria-required': required, 'aria-invalid': errors.some((e) => e.severity === 'error') || unanswered }

  let input
  if (definition.type === 'boolean') {
    const value = (answer as { valueBoolean?: boolean } | undefined)?.valueBoolean
    input =
      chosen === 'checkbox' ? (
        <input type="checkbox" {...common} checked={value === true} disabled={readOnly} onChange={(e) => set([{ valueBoolean: e.target.checked }])} />
      ) : (
        <div role="radiogroup" aria-labelledby={`${id}-label`} className={css({ display: 'flex', gap: '3' })}>
          {[true, false].map((option) => (
            <label key={String(option)} className={css({ display: 'flex', gap: '1', alignItems: 'center' })}>
              <input type="radio" name={id} checked={value === option} disabled={readOnly} onChange={() => set([{ valueBoolean: option }])} />
              {intl.formatMessage(option ? formMessages.fmYes : formMessages.fmNo)}
            </label>
          ))}
        </div>
      )
  } else if (definition.type === 'coding') {
    const options = (definition.answerOption ?? []).flatMap((o) => (o.valueCoding === undefined ? [] : [o.valueCoding]))
    const code = (answer as { valueCoding?: { code?: string } } | undefined)?.valueCoding?.code ?? ''
    const pick = (value: string) => {
      const coding = options.find((o) => o.code === value)
      set(coding === undefined ? [] : [{ valueCoding: coding }])
    }
    input =
      chosen === 'radio' ? (
        <div role="radiogroup" aria-labelledby={`${id}-label`} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          {options.map((o) => (
            <label key={o.code} className={css({ display: 'flex', gap: '1', alignItems: 'center' })}>
              <input type="radio" name={id} checked={code === o.code} disabled={readOnly} onChange={() => pick(o.code ?? '')} />
              {o.display ?? o.code}
            </label>
          ))}
        </div>
      ) : (
        <select {...common} className={control} value={code} disabled={readOnly} onChange={(e) => pick(e.target.value)}>
          <option value="">{intl.formatMessage(formMessages.fmChoose)}</option>
          {options.map((o) => (
            <option key={o.code} value={o.code}>
              {o.display ?? o.code}
            </option>
          ))}
        </select>
      )
  } else if (definition.type === 'integer' || definition.type === 'decimal') {
    const shownText = text ?? answerText(answer)
    const pattern = definition.type === 'integer' ? INTEGER : NUMBER
    input = (
      <input
        {...common}
        className={control}
        inputMode={definition.type === 'integer' ? 'numeric' : 'decimal'}
        value={shownText}
        readOnly={readOnly}
        onChange={(e) => {
          const next = e.target.value.trim()
          setText(e.target.value)
          if (next === '') set([])
          else if (pattern.test(next)) {
            // Decimals keep the text as typed (`1.50`), as a LosslessNumber (ADR 0052).
            const value = new LosslessNumber(next)
            set([definition.type === 'integer' ? { valueInteger: value } : { valueDecimal: value }])
          }
        }}
      />
    )
  } else if (definition.type === 'date' || definition.type === 'dateTime') {
    const key = definition.type === 'date' ? 'valueDate' : 'valueDateTime'
    input = (
      <input
        {...common}
        className={control}
        type={definition.type === 'date' ? 'date' : 'datetime-local'}
        value={answerText(answer)}
        readOnly={readOnly}
        onChange={(e) => set(e.target.value === '' ? [] : [{ [key]: e.target.value } as QuestionnaireResponseItemAnswer])}
      />
    )
  } else {
    const Tag = chosen === 'textarea' ? 'textarea' : 'input'
    input = (
      <Tag {...common} className={control} value={answerText(answer)} readOnly={readOnly} onChange={(e) => set(e.target.value === '' ? [] : [{ valueString: e.target.value }])} />
    )
  }

  const numeric = (definition.type === 'integer' || definition.type === 'decimal') && text !== undefined && text.trim() !== '' && !(definition.type === 'integer' ? INTEGER : NUMBER).test(text.trim())
  const grouped = definition.type === 'boolean' && chosen === 'radio'
  const groupedChoice = definition.type === 'coding' && chosen === 'radio'
  return (
    <div className={field}>
      {grouped || groupedChoice ? (
        <span id={`${id}-label`}>
          {label}
          {required ? ` (${intl.formatMessage(formMessages.fmRequired)})` : ''}
        </span>
      ) : (
        <label id={`${id}-label`} htmlFor={id}>
          {label}
          {required ? ` (${intl.formatMessage(formMessages.fmRequired)})` : ''}
        </label>
      )}
      {input}
      {described === undefined && !numeric ? null : (
        <div id={help} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          {computed === undefined ? null : <span className={quiet}>{intl.formatMessage(formMessages.fmCalculated)}</span>}
          {hint === undefined ? null : <span className={quiet}>{hint}</span>}
          {numeric ? <span className={danger}>{intl.formatMessage(formMessages.fmNotNumber)}</span> : null}
          {unanswered ? <span className={danger}>{intl.formatMessage(formMessages.fmRequired)}</span> : null}
          {errors.map((v) => (
            <span key={v.key} className={v.severity === 'error' ? danger : quiet}>
              {v.human}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

/** The Questionnaire items by linkId (re-exported for screens that list a form's questions). */
export { itemsByLinkId }
