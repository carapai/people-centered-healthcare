import { css, cx } from '@health/design-system/css'
import type { Coding, Questionnaire, QuestionnaireItem } from '@health/fhir-types'
import { useQueries, useQuery } from '@tanstack/react-query'
import { useDeferredValue, useId, useState } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { studioMessages as m, terminologyMessages as t } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import type { Transport } from '../../../platform/types'
import { widget as shownWidget, type FormLayout } from '../../forms/layout'
import { control } from '../../reception/styles'
import { StudioError } from '../api'
import { expandValueSet, terminologyKey, valueSetsQuery, type StudioExpansion, type StudioValueSet } from '../terminology/api'
import { bindingOf, suggestWidget } from '../terminology/model'
import { refusalText } from '../wording'
import { boundValueSets, layoutWidget, splitBinding, withPresentation, withValueSet } from './model'

// Binding a ValueSet from the form designer (studio.md § Terminology, Binding from the form
// designer; #497 part 3): the picker searches the ValueSets this package version may bind
// (published, or drafts in the same package), previews the codes, suggests a widget, and
// records `url|version`. The expansion is read from the terminology service (#523).

const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const row = css({ display: 'flex', alignItems: 'center', gap: '1', flexWrap: 'wrap' })
const input = cx(control, css({ width: '100%' }))
const box = css({ display: 'flex', flexDirection: 'column', gap: '2', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '2' })
const small = css({
  minHeight: 'control',
  minWidth: 'control',
  px: '2',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  textAlign: 'left',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
  _pressed: { borderColor: 'accent.default', fontWeight: 'semibold' },
})

/** How many codes a preview lists. */
const PREVIEWED = 10

/** The expansion of the ValueSet `binding` names (`url|version`), its first 1000 codes. */
function bindingQuery(transport: Transport, id: string, binding: string) {
  return {
    queryKey: [...terminologyKey(id), 'binding', binding] as const,
    queryFn: ({ signal }: { signal?: AbortSignal }): Promise<StudioExpansion> => expandValueSet(transport, id, splitBinding(binding), { count: 1000 }, signal),
    retry: false,
  }
}

/** The codes of every ValueSet the form binds, by binding, for the live preview. */
export function useBoundCodes(id: string, questionnaire: Questionnaire): Record<string, Coding[]> {
  const platform = usePlatform()
  const bindings = boundValueSets(questionnaire)
  const expansions = useQueries({ queries: bindings.map((binding) => bindingQuery(platform.transport, id, binding)) })
  return Object.fromEntries(
    bindings.flatMap((binding, n) => {
      const data = expansions[n]?.data
      return data === undefined ? [] : [[binding, data.contains.map((c) => ({ system: c.system, ...(c.version === null || c.version === undefined ? {} : { version: c.version }), code: c.code, ...(c.display === null || c.display === undefined ? {} : { display: c.display }) }))]]
    }),
  )
}

const failure = (intl: IntlShape, error: unknown) =>
  error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail : refusalText(intl, error.refusal)) : String(error)

const widgetName = (intl: IntlShape, widget: 'radio' | 'select' | 'search') => intl.formatMessage(m.fdWidgetName, { widget })

/** A ValueSet's codes in brief: how many, the first few, and the widget they suggest. */
function Codes({ id, binding, item, layout, editable, onEditLayout }: { id: string; binding: string; item: QuestionnaireItem; layout: FormLayout | undefined; editable: boolean; onEditLayout?: (f: (layout: FormLayout) => FormLayout) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const expansion = useQuery(bindingQuery(platform.transport, id, binding))
  if (expansion.isPending) return <p className={muted}>{intl.formatMessage(m.fdCodesLoading)}</p>
  if (expansion.isError) return <p className={alert}>{intl.formatMessage(t.tbExpansionFailed, { reason: failure(intl, expansion.error) })}</p>
  const { total, contains } = expansion.data
  const suggested = suggestWidget(total)
  const apply = layoutWidget(total)
  const current = shownWidget(item, layout)
  const shown = contains.slice(0, PREVIEWED)
  return (
    <>
      <p>{intl.formatMessage(t.tbExpansion, { total })}</p>
      <ul aria-label={intl.formatMessage(m.fdCodesOf, { binding })} className={css({ display: 'flex', flexDirection: 'column', gap: '0.5', fontSize: 'sm' })}>
        {shown.map((c) => (
          <li key={`${c.system}|${c.code}`}>
            <code>{c.code}</code> {c.display ?? ''}
          </li>
        ))}
      </ul>
      {total > shown.length ? <p className={muted}>{intl.formatMessage(m.fdCodesMore, { more: total - shown.length })}</p> : null}
      <p>{intl.formatMessage(m.fdSuggested, { widget: widgetName(intl, suggested) })}</p>
      {suggested === 'search' ? <p className={muted}>{intl.formatMessage(m.fdSuggestedSearch)}</p> : null}
      {onEditLayout !== undefined && editable && current !== apply ? (
        <button type="button" className={small} onClick={() => onEditLayout((l) => withPresentation(l, item.linkId, { widget: apply }))}>
          {intl.formatMessage(m.fdUseSuggested, { widget: widgetName(intl, apply === 'radio' ? 'radio' : 'select') })}
        </button>
      ) : null}
    </>
  )
}

/** Where a ValueSet comes from: a draft of this package, or a published package. */
const origin = (intl: IntlShape, vs: StudioValueSet) => (vs.own ? intl.formatMessage(m.fdPickerOwn) : intl.formatMessage(m.fdPickerPublished, { package: vs.package }))

const title = (vs: StudioValueSet) => vs.title ?? vs.url

/** The picker: search, choose, preview, bind. */
function Picker({ id, item, layout, onBind, onCancel }: { id: string; item: QuestionnaireItem; layout: FormLayout | undefined; onBind: (binding: string) => void; onCancel: () => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const hint = useId()
  const [text, setText] = useState('')
  const searched = useDeferredValue(text.trim())
  const [chosen, setChosen] = useState<StudioValueSet | null>(null)
  const found = useQuery({ ...valueSetsQuery(platform.transport, id, searched), retry: false, placeholderData: (previous) => previous })
  const binding = chosen === null ? null : bindingOf(chosen.url, chosen.version)
  return (
    <div role="group" aria-label={intl.formatMessage(m.fdPicker)} className={box}>
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        {intl.formatMessage(m.fdPickerSearch)}
        <input className={input} type="search" value={text} aria-describedby={hint} onChange={(e) => setText(e.target.value)} />
        <span id={hint} className={muted}>
          {intl.formatMessage(m.fdPickerHint)}
        </span>
      </label>
      {found.isError ? (
        <p className={alert}>{intl.formatMessage(m.fdPickerFailed, { reason: failure(intl, found.error) })}</p>
      ) : found.data === undefined ? (
        <p className={muted}>{intl.formatMessage(m.fdCodesLoading)}</p>
      ) : found.data.length === 0 ? (
        <p className={muted}>{intl.formatMessage(m.fdPickerNone)}</p>
      ) : (
        <ul aria-label={intl.formatMessage(m.fdPickerResults)} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          {found.data.map((vs) => (
            <li key={`${vs.package}|${vs.url}|${vs.version ?? ''}`}>
              <button type="button" className={cx(small, css({ width: '100%' }))} aria-pressed={chosen !== null && chosen.url === vs.url && chosen.version === vs.version && chosen.package === vs.package} onClick={() => setChosen(vs)}>
                {title(vs)} {vs.version ?? ''} · {origin(intl, vs)}
              </button>
            </li>
          ))}
        </ul>
      )}
      {chosen === null || binding === null ? null : (
        <>
          <p className={muted}>{binding}</p>
          <Codes id={id} binding={binding} item={item} layout={layout} editable={false} />
          {(item.answerOption ?? []).length > 0 ? <p className={muted}>{intl.formatMessage(m.fdBindReplaces)}</p> : null}
        </>
      )}
      <div className={row}>
        <button type="button" className={small} disabled={binding === null} onClick={() => binding !== null && onBind(binding)}>
          {chosen === null ? intl.formatMessage(m.fdBindNone) : intl.formatMessage(m.fdBind, { title: title(chosen) })}
        </button>
        <button type="button" className={small} onClick={onCancel}>
          {intl.formatMessage(m.fdPickerCancel)}
        </button>
      </div>
    </div>
  )
}

/**
 * A choice question's bound ValueSet, with its codes and suggested widget, or the picker that
 * binds one. A question offers its own options or a ValueSet's codes, never both.
 */
export function ValueSetBinding({
  id,
  item,
  layout,
  editable,
  onEdit,
  onEditLayout,
}: {
  id: string
  item: QuestionnaireItem
  layout: FormLayout | undefined
  editable: boolean
  onEdit: (f: (item: QuestionnaireItem) => QuestionnaireItem) => void
  onEditLayout: (f: (layout: FormLayout) => FormLayout) => void
}) {
  const intl = useIntl()
  const [picking, setPicking] = useState(false)
  const bound = item.answerValueSet
  const picker = (
    <Picker
      id={id}
      item={item}
      layout={layout}
      onCancel={() => setPicking(false)}
      onBind={(binding) => {
        onEdit((x) => withValueSet(x, binding))
        setPicking(false)
      }}
    />
  )
  if (bound === undefined)
    return editable ? (
      picking ? (
        picker
      ) : (
        <button type="button" className={small} onClick={() => setPicking(true)}>
          {intl.formatMessage(m.fdBindValueSet)}
        </button>
      )
    ) : null
  return (
    <div className={box}>
      <p>{intl.formatMessage(m.fdValueSet, { valueSet: bound })}</p>
      <Codes id={id} binding={bound} item={item} layout={layout} editable={editable} onEditLayout={onEditLayout} />
      {editable ? (
        picking ? (
          picker
        ) : (
          <div className={row}>
            <button type="button" className={small} onClick={() => setPicking(true)}>
              {intl.formatMessage(m.fdChangeValueSet)}
            </button>
            <button type="button" className={small} onClick={() => onEdit((x) => withValueSet(x, null))}>
              {intl.formatMessage(m.fdUnbind)}
            </button>
          </div>
        )
      ) : null}
    </div>
  )
}
