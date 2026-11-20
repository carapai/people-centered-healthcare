import { css, cx } from '@health/design-system/css'
import type { Questionnaire, QuestionnaireItem } from '@health/fhir-types'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useId, useMemo, useRef, useState } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { studioMessages as m } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { FORM_LAYOUT_SCHEMA, fitting, type FormLayout, type Widget } from '../../forms/layout'
import { QuestionnaireRenderer } from '../../forms/QuestionnaireRenderer'
import { button, control } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { refusalText } from '../wording'
import {
  ANSWER_TYPES,
  addOption,
  addQuestion,
  addSection,
  aligned,
  blankLayout,
  editOption,
  itemAt,
  laysOut,
  linkIds,
  moveOption,
  moveSection,
  moveShown,
  outline,
  problems,
  removeItem,
  removeOption,
  removeSection,
  renameItem,
  renameSection,
  rows,
  sectionOf,
  shownWhen,
  updateItem,
  withFlag,
  withPresentation,
  withSection,
  withShownWhen,
  withType,
  type AnswerType,
  type Path,
  type Problem,
} from './model'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const panel = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface', p: '3', display: 'flex', flexDirection: 'column', gap: '3', minWidth: '0' })
const columns = css({ display: 'grid', gap: '4', alignItems: 'start', gridTemplateColumns: { base: '1fr', lg: '18rem 24rem minmax(0, 1fr)' } })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1' })
const input = cx(control, css({ width: '100%' }))
const fieldGroup = css({ display: 'flex', flexDirection: 'column', gap: '2', border: 'none', p: '0', m: '0' })
const legend = css({ fontWeight: 'semibold', mb: '1' })
const row = css({ display: 'flex', alignItems: 'center', gap: '1' })
const small = css({
  minHeight: 'control',
  minWidth: 'control',
  px: '2',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})

/** The Form Layout file of `questionnaire` among the draft's configuration files, if any. */
function findLayout(files: { path: string; file: StudioFile | undefined }[], questionnaire: Questionnaire): { path: string; layout: FormLayout } | undefined {
  for (const { path, file } of files) {
    if (file === undefined) continue
    try {
      const value = JSON.parse(file.content) as Partial<FormLayout>
      if (value.$schema === FORM_LAYOUT_SCHEMA && typeof value.questionnaire === 'string' && value.questionnaire.split('|')[0] === questionnaire.url) return { path, layout: value as FormLayout }
    } catch {
      // Not JSON: not a layout.
    }
  }
  return undefined
}

/** Where a new Form Layout for the Questionnaire at `path` goes. */
const newLayoutPath = (path: string) => `configuration/FormLayout-${(path.split('/').pop() ?? path).replace(/^Questionnaire-/, '').replace(/\.json$/, '')}.json`

function parseQuestionnaire(content: string): Questionnaire | null {
  try {
    const value = JSON.parse(content) as { resourceType?: unknown }
    return value.resourceType === 'Questionnaire' ? (value as Questionnaire) : null
  } catch {
    return null
  }
}

/**
 * The form designer (studio.md § The form designer; #497): a draft's Questionnaire edited in
 * three columns, the questions by Form Layout section on the left, the selected question's
 * inspector in the middle and a live preview in the real `QuestionnaireRenderer` on the right,
 * with the problems found while editing above. It saves through the same path as the file
 * editor (#494): each file with the draft's revision as `If-Match`, so a stale tab is refused
 * with 412 and keeps nothing. A question published in the previous version keeps its id.
 */
export function FormDesigner({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  const file = useQuery({ ...fileQuery(platform.transport, id, path), retry: false })
  const previousId = draft.data?.package.previous === null || draft.data?.package.previous === undefined ? null : `${draft.data.package.name}@${draft.data.package.previous}`
  const previous = useQuery({ ...fileQuery(platform.transport, previousId ?? '', path), enabled: previousId !== null, retry: false })
  const configuration = (draft.data?.files ?? []).filter((f) => f.path.startsWith('configuration/') && f.path.endsWith('.json')).map((f) => f.path)
  const layouts = useQueries({ queries: configuration.map((p) => ({ ...fileQuery(platform.transport, id, p), retry: false })) })
  // A previous version without this file published none of its ids.
  const previousSettled = previousId === null || previous.data !== undefined || (previous.error instanceof StudioError && previous.error.status === 404)
  // Once the designer has opened it stays open: a save that adds the layout file adds a query,
  // and only *Reload* after a refused save opens it again, from what the server holds.
  const opened = useRef(false)
  const [reloads, setReloads] = useState(0)
  const loading = !opened.current && (draft.data === undefined || file.data === undefined || !previousSettled || layouts.some((q) => q.isPending))
  const failed = draft.isError || file.isError || (previous.isError && !previousSettled)
  if (!loading && draft.data !== undefined && file.data !== undefined) opened.current = true
  return (
    <section className={page}>
      <div className={css({ display: 'flex', gap: '4', flexWrap: 'wrap' })}>
        <Link to="/studio/$id" params={{ id }}>
          {intl.formatMessage(m.sdEditorBack)}
        </Link>
        <Link to="/studio/$id/files/$" params={{ id, _splat: path }}>
          {intl.formatMessage(m.fdAsJson)}
        </Link>
      </div>
      {loading || draft.data === undefined || file.data === undefined ? (
        failed ? (
          <p role="alert" className={alert}>
            {intl.formatMessage(m.sdFailed)}
          </p>
        ) : (
          <p role="status">{intl.formatMessage(m.fdLoading)}</p>
        )
      ) : (
        <Loaded
          key={reloads}
          id={id}
          path={path}
          draft={draft.data}
          file={file.data}
          published={previous.data === undefined ? null : parseQuestionnaire(previous.data.content)}
          layouts={configuration.map((p, n) => ({ path: p, file: layouts[n]?.data }))}
          onReload={async () => {
            await Promise.all([draft.refetch(), file.refetch(), ...layouts.map((q) => q.refetch())])
            setReloads((n) => n + 1)
          }}
        />
      )}
    </section>
  )
}

interface LoadedProps {
  id: string
  path: string
  draft: StudioPackagePage
  file: StudioFile
  published: Questionnaire | null
  layouts: { path: string; file: StudioFile | undefined }[]
  onReload: () => Promise<void>
}

function Loaded(props: LoadedProps) {
  const intl = useIntl()
  const questionnaire = parseQuestionnaire(props.file.content)
  if (questionnaire === null)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.fdNotQuestionnaire)}
      </p>
    )
  return <Designer {...props} initial={questionnaire} />
}

interface Form {
  questionnaire: Questionnaire
  layout: FormLayout | undefined
}

function Designer({ id, path, draft, file, published: previous, layouts, onReload, initial }: LoadedProps & { initial: Questionnaire }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const found = useMemo(() => findLayout(layouts, initial), [layouts, initial])
  const layoutPath = found?.path ?? newLayoutPath(path)
  const [form, setForm] = useState<Form>({ questionnaire: initial, layout: found?.layout })
  const [saved, setSaved] = useState(() => ({ questionnaire: JSON.stringify(initial), layout: JSON.stringify(found?.layout ?? null) }))
  const [layoutExists, setLayoutExists] = useState(found !== undefined)
  const [revision, setRevision] = useState(file.revision)
  const [selected, setSelected] = useState<Path | null>(() => ((initial.item ?? []).length > 0 ? [0] : null))
  const published = useMemo(() => linkIds(previous ?? undefined), [previous])
  const editable = draft.editable

  const layout = form.layout === undefined ? undefined : aligned(form.layout, form.questionnaire)
  const questionnaireText = JSON.stringify(form.questionnaire)
  const layoutText = JSON.stringify(form.layout ?? null)
  const layoutDirty = layoutText !== saved.layout && (layoutExists || (layout !== undefined && laysOut(layout)))
  const dirty = questionnaireText !== saved.questionnaire || layoutDirty
  const listed = problems(form.questionnaire, layout, published)

  const save = useMutation({
    mutationFn: async () => {
      let next = await saveFile(platform.transport, id, path, JSON.stringify(form.questionnaire, null, 2), revision)
      // The layout follows in a second save, at the revision the first one answered.
      if (layoutDirty && layout !== undefined) next = await saveFile(platform.transport, id, layoutPath, JSON.stringify(layout, null, 2), next.revision)
      return next
    },
    onSuccess: async (next) => {
      setRevision(next.revision)
      setSaved({ questionnaire: questionnaireText, layout: layoutText })
      if (layoutDirty) setLayoutExists(true)
      // What was saved is what the server now holds, for the JSON editor and the next opening.
      queryClient.setQueryData(fileQuery(platform.transport, id, path).queryKey, { path, content: JSON.stringify(form.questionnaire, null, 2), revision: next.revision })
      if (layoutDirty && layout !== undefined) queryClient.setQueryData(fileQuery(platform.transport, id, layoutPath).queryKey, { path: layoutPath, content: JSON.stringify(layout, null, 2), revision: next.revision })
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
    },
  })
  const stale = save.error instanceof StudioError && save.error.status === 412
  const reason =
    save.error instanceof StudioError ? (save.error.refusal === null || save.error.refusal === undefined ? save.error.detail : refusalText(intl, save.error.refusal)) : save.error === null ? '' : String(save.error)

  const change = (next: Form) => {
    setForm(next)
    save.reset()
  }
  const edit = (at: Path, f: (item: QuestionnaireItem) => QuestionnaireItem) => change({ ...form, questionnaire: updateItem(form.questionnaire, at, f) })
  const editLayout = (f: (l: FormLayout) => FormLayout) => change({ ...form, layout: f(form.layout ?? blankLayout(form.questionnaire)) })
  const selectedItem = selected === null ? undefined : itemAt(form.questionnaire, selected)

  return (
    <>
      <header className={css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center', justifyContent: 'space-between' })}>
        <div>
          <h1 className={heading}>{intl.formatMessage(m.fdHeading, { title: form.questionnaire.title ?? form.questionnaire.name ?? path })}</h1>
          <p className={muted}>
            {path} · {intl.formatMessage(layoutExists ? m.fdLayoutFile : m.fdLayoutNew, { path: layoutPath })}
          </p>
        </div>
        {editable ? (
          <div className={row}>
            {dirty ? <span className={muted}>{intl.formatMessage(m.fdUnsaved)}</span> : null}
            <button type="button" className={button} disabled={!dirty || save.isPending || stale} onClick={() => save.mutate()}>
              {intl.formatMessage(m.fdSave)}
            </button>
          </div>
        ) : null}
      </header>
      {editable ? null : (
        <p className={muted}>
          {draft.editRefusal === null || draft.editRefusal === undefined ? intl.formatMessage(m.sdEditorReadOnly) : intl.formatMessage(m.sdEditRefused, { reason: refusalText(intl, draft.editRefusal) })}
        </p>
      )}
      {save.isSuccess && !dirty ? <p role="status">{intl.formatMessage(m.fdSaved)}</p> : null}
      {stale ? (
        <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
          <p className={alert}>{intl.formatMessage(m.sdEditorStale)}</p>
          <button type="button" className={button} onClick={() => void onReload()}>
            {intl.formatMessage(m.sdEditorReload)}
          </button>
        </div>
      ) : save.error === null ? null : (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.sdEditorFailed, { reason })}
        </p>
      )}

      <Problems questionnaire={form.questionnaire} found={listed} onSelect={setSelected} />

      <div className={columns}>
        <Outline form={form} layout={layout} selected={selected} editable={editable} onSelect={setSelected} onChange={change} onEditLayout={editLayout} />
        <section aria-labelledby="fd-inspector" className={panel}>
          <h2 id="fd-inspector" className={subheading}>
            {intl.formatMessage(m.fdInspector)}
          </h2>
          {selected === null || selectedItem === undefined ? (
            <p className={muted}>{intl.formatMessage(m.fdChoose)}</p>
          ) : (
            <Inspector
              key={selected.join('.')}
              form={form}
              layout={layout}
              path={selected}
              item={selectedItem}
              published={published}
              editable={editable}
              onEdit={(f) => edit(selected, f)}
              onRename={(linkId) => change(renameItem(form, selected, linkId, published))}
              onEditLayout={editLayout}
              onRemove={() => {
                change({ questionnaire: removeItem(form.questionnaire, selected), layout: form.layout === undefined ? undefined : withSection(form.layout, selectedItem.linkId, null) })
                setSelected(null)
              }}
            />
          )}
        </section>
        <section aria-labelledby="fd-preview" className={panel}>
          <h2 id="fd-preview" className={subheading}>
            {intl.formatMessage(m.fdPreview)}
          </h2>
          {/* Remounted on every change, so it always shows the form as it now stands. */}
          <QuestionnaireRenderer key={questionnaireText + layoutText} questionnaire={form.questionnaire} {...(layout === undefined ? {} : { layout })} onSubmit={() => undefined} />
        </section>
      </div>
    </>
  )
}

/** The text an author knows a question by. */
const known = (intl: IntlShape, item: QuestionnaireItem | undefined, linkId: string) => {
  const text = item?.text?.trim() ?? ''
  return text === '' ? intl.formatMessage(m.fdUntitled, { id: linkId }) : text
}

function problemText(intl: IntlShape, questionnaire: Questionnaire, problem: Problem): string {
  const text = 'path' in problem ? known(intl, itemAt(questionnaire, problem.path), 'linkId' in problem ? problem.linkId : '') : ''
  switch (problem.code) {
    case 'no-id':
      return intl.formatMessage(m.fdPNoId)
    case 'duplicate-id':
      return intl.formatMessage(m.fdPDuplicate, { id: problem.linkId })
    case 'no-text':
      return intl.formatMessage(m.fdPNoText, { id: problem.linkId })
    case 'no-options':
      return intl.formatMessage(m.fdPNoOptions, { text })
    case 'condition-not-yes-no':
      return intl.formatMessage(m.fdPCondition, { text })
    case 'widget-misfit':
      return intl.formatMessage(m.fdPWidget, { text, widget: problem.widget })
    case 'no-section':
      return intl.formatMessage(m.fdPNoSection, { text })
    case 'published-removed':
      return intl.formatMessage(m.fdPRemoved, { id: problem.linkId })
    case 'section-no-title':
      return intl.formatMessage(m.fdPSectionTitle, { n: problem.section + 1 })
  }
}

/** The problems found while editing, each selecting the question it is about. */
function Problems({ questionnaire, found, onSelect }: { questionnaire: Questionnaire; found: Problem[]; onSelect: (path: Path) => void }) {
  const intl = useIntl()
  return (
    <section aria-labelledby="fd-problems" className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <h2 id="fd-problems" className={subheading}>
        {intl.formatMessage(m.fdProblems, { count: found.length })}
      </h2>
      <p className={muted}>{intl.formatMessage(m.fdProblemsHint)}</p>
      {found.length === 0 ? null : (
        <ul className={css({ listStyle: 'disc', pl: '5', display: 'flex', flexDirection: 'column', gap: '1' })}>
          {found.map((problem, n) => {
            const text = problemText(intl, questionnaire, problem)
            return (
              <li key={`${problem.code}:${n}`}>
                {'path' in problem ? (
                  <button type="button" className={css({ textDecoration: 'underline', cursor: 'pointer', textAlign: 'left' })} onClick={() => onSelect(problem.path)}>
                    {text}
                  </button>
                ) : (
                  text
                )}
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}

interface OutlineProps {
  form: Form
  layout: FormLayout | undefined
  selected: Path | null
  editable: boolean
  onSelect: (path: Path) => void
  onChange: (form: Form) => void
  onEditLayout: (f: (layout: FormLayout) => FormLayout) => void
}

const same = (a: Path | null, b: Path) => a !== null && a.length === b.length && a.every((x, n) => x === b[n])

/** The questions by section, those in no section last; reordering, adding and the sections. */
function Outline({ form, layout, selected, editable, onSelect, onChange, onEditLayout }: OutlineProps) {
  const intl = useIntl()
  const add = () => {
    const added = addQuestion(form.questionnaire, intl.formatMessage(m.fdNewQuestion))
    const linkId = itemAt(added.questionnaire, added.path)?.linkId ?? ''
    // It joins the section of the selected question, if that is in one.
    const top = selected === null ? undefined : itemAt(form.questionnaire, selected.slice(0, 1))
    const section = top === undefined || layout === undefined ? null : sectionOf(layout, top.linkId)
    onChange({ questionnaire: added.questionnaire, layout: section === null || form.layout === undefined ? form.layout : withSection(form.layout, linkId, section) })
    onSelect(added.path)
  }
  return (
    <nav aria-labelledby="fd-questions" className={panel}>
      <h2 id="fd-questions" className={subheading}>
        {intl.formatMessage(m.fdQuestions)}
      </h2>
      {outline(form.questionnaire, layout).map((group) =>
        group.section === null && group.rows.length === 0 && (layout?.sections.length ?? 0) > 0 ? null : (
          <div key={group.section ?? 'none'} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
            {group.section === null && (layout?.sections.length ?? 0) === 0 ? null : (
              <h3 className={cx(muted, css({ fontWeight: 'semibold' }))}>{group.title ?? intl.formatMessage(m.fdNoSection)}</h3>
            )}
            <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              {group.rows.map(({ path, item, depth }) => {
                const text = known(intl, item, item.linkId)
                return (
                  <li key={path.join('.')} className={row} style={{ paddingLeft: `${depth * 1.25}rem` }}>
                    <button
                      type="button"
                      aria-current={same(selected, path) ? 'true' : undefined}
                      className={css({ flex: '1', textAlign: 'left', p: '1', borderRadius: 'sm', cursor: 'pointer', minWidth: '0', overflowWrap: 'anywhere', '&[aria-current=true]': { bg: 'accent.subtle', fontWeight: 'semibold' } })}
                      onClick={() => onSelect(path)}
                    >
                      {shownWhen(item) === null ? '' : '↳ '}
                      {text} <span className={muted}>{item.type}</span>
                    </button>
                    {editable ? (
                      <>
                        <button
                          type="button"
                          className={small}
                          aria-label={intl.formatMessage(m.fdUp, { text })}
                          onClick={() => {
                            const moved = moveShown(form, path, -1)
                            onChange({ questionnaire: moved.questionnaire, layout: moved.layout })
                            onSelect(moved.path)
                          }}
                        >
                          ↑
                        </button>
                        <button
                          type="button"
                          className={small}
                          aria-label={intl.formatMessage(m.fdDown, { text })}
                          onClick={() => {
                            const moved = moveShown(form, path, 1)
                            onChange({ questionnaire: moved.questionnaire, layout: moved.layout })
                            onSelect(moved.path)
                          }}
                        >
                          ↓
                        </button>
                      </>
                    ) : null}
                  </li>
                )
              })}
            </ul>
          </div>
        ),
      )}
      {editable ? (
        <button type="button" className={small} onClick={add}>
          {intl.formatMessage(m.fdAddQuestion)}
        </button>
      ) : null}
      <Sections layout={layout} editable={editable} onEditLayout={onEditLayout} />
    </nav>
  )
}

function Sections({ layout, editable, onEditLayout }: { layout: FormLayout | undefined; editable: boolean; onEditLayout: (f: (layout: FormLayout) => FormLayout) => void }) {
  const intl = useIntl()
  const list = layout?.sections ?? []
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2', borderTop: '1px solid', borderColor: 'border', pt: '3' })}>
      <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.fdSections)}</h3>
      {list.map((section, n) => (
        <div key={n} className={row}>
          <input
            className={input}
            aria-label={intl.formatMessage(m.fdSectionTitle, { n: n + 1 })}
            value={section.title}
            readOnly={!editable}
            onChange={(e) => onEditLayout((l) => renameSection(l, n, e.target.value))}
          />
          {editable ? (
            <>
              <button type="button" className={small} aria-label={intl.formatMessage(m.fdSectionUp, { title: section.title })} onClick={() => onEditLayout((l) => moveSection(l, n, -1))}>
                ↑
              </button>
              <button type="button" className={small} aria-label={intl.formatMessage(m.fdSectionDown, { title: section.title })} onClick={() => onEditLayout((l) => moveSection(l, n, 1))}>
                ↓
              </button>
              <button type="button" className={small} aria-label={intl.formatMessage(m.fdSectionRemove, { title: section.title })} onClick={() => onEditLayout((l) => removeSection(l, n))}>
                ×
              </button>
            </>
          ) : null}
        </div>
      ))}
      {editable ? (
        <button type="button" className={small} onClick={() => onEditLayout((l) => addSection(l, intl.formatMessage(m.fdNewSection, { n: l.sections.length + 1 })))}>
          {intl.formatMessage(m.fdAddSection)}
        </button>
      ) : null}
    </div>
  )
}

interface InspectorProps {
  form: Form
  layout: FormLayout | undefined
  path: Path
  item: QuestionnaireItem
  published: ReadonlySet<string>
  editable: boolean
  onEdit: (f: (item: QuestionnaireItem) => QuestionnaireItem) => void
  onRename: (linkId: string) => void
  onEditLayout: (f: (layout: FormLayout) => FormLayout) => void
  onRemove: () => void
}

/** The selected question in three groups: Question, Layout, and Becomes a record. */
function Inspector({ form, layout, path, item, published, editable, onEdit, onRename, onEditLayout, onRemove }: InspectorProps) {
  const intl = useIntl()
  const idHint = useId()
  const fixed = published.has(item.linkId)
  const editableType = (ANSWER_TYPES as readonly string[]).includes(item.type)
  const condition = shownWhen(item)
  const yesNo = rows(form.questionnaire).filter((r) => r.item.type === 'boolean' && r.item.linkId !== item.linkId)
  const allowed = fitting(item)
  const text = known(intl, item, item.linkId)
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <fieldset className={fieldGroup} disabled={!editable}>
        <legend className={legend}>{intl.formatMessage(m.fdQuestion)}</legend>
        <label className={label}>
          {intl.formatMessage(m.fdText)}
          <input className={input} value={item.text ?? ''} onChange={(e) => onEdit((x) => ({ ...x, text: e.target.value }))} />
        </label>
        <label className={label}>
          {intl.formatMessage(m.fdId)}
          <input className={input} value={item.linkId} readOnly={fixed} aria-describedby={idHint} onChange={(e) => onRename(e.target.value)} />
          <span id={idHint} className={muted}>
            {intl.formatMessage(fixed ? m.fdIdFixed : m.fdIdNew)}
          </span>
        </label>
        {editableType ? (
          <label className={label}>
            {intl.formatMessage(m.fdType)}
            <select className={input} value={item.type} onChange={(e) => onEdit((x) => withType(x, e.target.value as AnswerType))}>
              {ANSWER_TYPES.map((t) => (
                <option key={t} value={t}>
                  {t === 'coding' ? 'choice' : t}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <p className={muted}>{intl.formatMessage(m.fdTypeOther, { type: item.type })}</p>
        )}
        {item.type === 'group' || item.type === 'display' ? null : (
          <label className={row}>
            <input type="checkbox" checked={item.required === true} onChange={(e) => onEdit((x) => withFlag(x, 'required', e.target.checked))} />
            {intl.formatMessage(m.fdRequired)}
          </label>
        )}
        {item.type === 'coding' ? (
          <>
            <label className={row}>
              <input type="checkbox" checked={item.repeats === true} onChange={(e) => onEdit((x) => withFlag(x, 'repeats', e.target.checked))} />
              {intl.formatMessage(m.fdRepeats)}
            </label>
            <Options item={item} onEdit={onEdit} editable={editable} />
          </>
        ) : null}
        {condition === 'other' ? (
          <p className={muted}>{intl.formatMessage(m.fdConditionOther)}</p>
        ) : (
          <label className={label}>
            {intl.formatMessage(m.fdShownWhen)}
            <select className={input} value={condition ?? ''} onChange={(e) => onEdit((x) => withShownWhen(x, e.target.value === '' ? null : e.target.value))}>
              <option value="">{intl.formatMessage(m.fdAlways)}</option>
              {condition !== null && !yesNo.some((r) => r.item.linkId === condition) ? <option value={condition}>{condition}</option> : null}
              {yesNo.map((r) => (
                <option key={r.path.join('.')} value={r.item.linkId}>
                  {intl.formatMessage(m.fdIsYes, { text: known(intl, r.item, r.item.linkId) })}
                </option>
              ))}
            </select>
          </label>
        )}
      </fieldset>

      <fieldset className={fieldGroup} disabled={!editable}>
        <legend className={legend}>{intl.formatMessage(m.fdLayout)}</legend>
        {path.length === 1 ? (
          <label className={label}>
            {intl.formatMessage(m.fdSection)}
            <select
              className={input}
              value={layout === undefined ? '' : String(sectionOf(layout, item.linkId) ?? '')}
              onChange={(e) => onEditLayout((l) => withSection(l, item.linkId, e.target.value === '' ? null : Number(e.target.value)))}
            >
              <option value="">{intl.formatMessage(m.fdSectionNone)}</option>
              {(layout?.sections ?? []).map((section, n) => (
                <option key={n} value={String(n)}>
                  {section.title}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <p className={muted}>{intl.formatMessage(m.fdSectionNested)}</p>
        )}
        <label className={label}>
          {intl.formatMessage(m.fdWidget)}
          <select className={input} value={layout?.items?.[item.linkId]?.widget ?? ''} onChange={(e) => onEditLayout((l) => withPresentation(l, item.linkId, { widget: e.target.value === '' ? null : (e.target.value as Widget) }))}>
            <option value="">{intl.formatMessage(m.fdWidgetDefault, { widget: allowed[0] ?? 'input' })}</option>
            {allowed.slice(1).map((w) => (
              <option key={w} value={w}>
                {w}
              </option>
            ))}
            {/* A widget chosen before that does not fit stays visible, and is listed as a problem. */}
            {(() => {
              const chosen = layout?.items?.[item.linkId]?.widget
              return chosen !== undefined && !allowed.slice(1).includes(chosen) && chosen !== allowed[0] ? <option value={chosen}>{chosen}</option> : null
            })()}
          </select>
        </label>
        <label className={label}>
          {intl.formatMessage(m.fdHint)}
          <input className={input} value={layout?.items?.[item.linkId]?.hint ?? ''} onChange={(e) => onEditLayout((l) => withPresentation(l, item.linkId, { hint: e.target.value }))} />
        </label>
      </fieldset>

      <div className={fieldGroup}>
        <h3 className={legend}>{intl.formatMessage(m.fdRecord)}</h3>
        <p className={muted}>{intl.formatMessage(m.fdRecordLater)}</p>
      </div>
      {editable ? (
        <button type="button" className={small} onClick={onRemove}>
          {intl.formatMessage(m.fdRemove, { text })}
        </button>
      ) : null}
    </div>
  )
}

function Options({ item, editable, onEdit }: { item: QuestionnaireItem; editable: boolean; onEdit: (f: (item: QuestionnaireItem) => QuestionnaireItem) => void }) {
  const intl = useIntl()
  const list = item.answerOption ?? []
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <h3 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.fdOptions)}</h3>
      {item.answerValueSet === undefined ? null : <p className={muted}>{intl.formatMessage(m.fdValueSet, { valueSet: item.answerValueSet })}</p>}
      <ol className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {list.map((option, n) =>
          option.valueCoding === undefined ? null : (
            <li key={n} className={row}>
              <input
                className={cx(control, css({ width: '7rem', flexShrink: '0' }))}
                aria-label={intl.formatMessage(m.fdOptionCode, { n: n + 1 })}
                value={option.valueCoding.code ?? ''}
                onChange={(e) => onEdit((x) => editOption(x, n, { code: e.target.value }))}
              />
              <input className={input} aria-label={intl.formatMessage(m.fdOptionDisplay, { n: n + 1 })} value={option.valueCoding.display ?? ''} onChange={(e) => onEdit((x) => editOption(x, n, { display: e.target.value }))} />
              {editable ? (
                <>
                  <button type="button" className={small} aria-label={intl.formatMessage(m.fdOptionUp, { n: n + 1 })} onClick={() => onEdit((x) => moveOption(x, n, -1))}>
                    ↑
                  </button>
                  <button type="button" className={small} aria-label={intl.formatMessage(m.fdOptionDown, { n: n + 1 })} onClick={() => onEdit((x) => moveOption(x, n, 1))}>
                    ↓
                  </button>
                  <button type="button" className={small} aria-label={intl.formatMessage(m.fdOptionRemove, { n: n + 1 })} onClick={() => onEdit((x) => removeOption(x, n))}>
                    ×
                  </button>
                </>
              ) : null}
            </li>
          ),
        )}
      </ol>
      {editable ? (
        <button type="button" className={small} onClick={() => onEdit((x) => addOption(x, intl.formatMessage(m.fdNewOption, { n: (x.answerOption ?? []).length + 1 })))}>
          {intl.formatMessage(m.fdAddOption)}
        </button>
      ) : null}
    </div>
  )
}
