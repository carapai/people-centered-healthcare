import { css, cx } from '@health/design-system/css'
import type { ValueSet } from '@health/fhir-types'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { terminologyMessages as m } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button, control } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioFile, type StudioPackagePage } from '../api'
import { refusalText } from '../wording'
import { AddCode, CodeSystemChange, EditCode, type Authoring } from './CodeSystemEditor'
import { codeSystemsQuery, conceptQuery, conceptsQuery, expandValueSet, terminologyKey, type StudioCodeSystem, type StudioConceptRow, type StudioExpansion } from './api'
import { codeKey, excludeCode, includeCode, includeUnder, newValueSet, removeListed, removeRule, rules, valueSetPath, type Rule } from './model'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const minor = css({ fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const panel = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface', p: '3', display: 'flex', flexDirection: 'column', gap: '3', minWidth: '0' })
const columns = css({ display: 'grid', gap: '4', alignItems: 'start', gridTemplateColumns: { base: '1fr', lg: '18rem minmax(0, 1fr) minmax(0, 1fr)' } })
const list = css({ display: 'flex', flexDirection: 'column', gap: '1', listStyle: 'none', p: '0', m: '0' })
const nested = css({ pl: '5', borderLeft: '1px solid', borderColor: 'border' })
const row = css({ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '2' })
const input = cx(control, css({ width: '100%' }))
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
  '&[aria-pressed=true]': { borderColor: 'accent.default', fontWeight: 'semibold' },
})
const tag = css({ fontSize: 'xs', px: '1', borderRadius: 'sm', border: '1px solid', borderColor: 'border' })
const removed = css({ color: 'danger.text' })

/** The page's ValueSet or CodeSystem, read from its file. */
type Opened = { kind: 'ValueSet'; valueSet: ValueSet } | { kind: 'CodeSystem'; url: string; version: string | undefined; title: string } | { kind: 'other' }

function opened(file: StudioFile | undefined): Opened {
  if (file === undefined) return { kind: 'other' }
  try {
    const value = JSON.parse(file.content) as { resourceType?: unknown; url?: unknown; version?: unknown; title?: unknown; name?: unknown }
    if (value.resourceType === 'ValueSet') return { kind: 'ValueSet', valueSet: value as ValueSet }
    if (value.resourceType === 'CodeSystem' && typeof value.url === 'string') {
      const title = typeof value.title === 'string' ? value.title : typeof value.name === 'string' ? value.name : value.url
      return { kind: 'CodeSystem', url: value.url, version: typeof value.version === 'string' ? value.version : undefined, title }
    }
  } catch {
    // Not JSON: not terminology.
  }
  return { kind: 'other' }
}

const reasonOf = (intl: ReturnType<typeof useIntl>, error: unknown) =>
  error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail : refusalText(intl, error.refusal)) : error === null || error === undefined ? '' : String(error)

/**
 * The terminology browser (studio.md § Terminology; #523), opened from a draft's CodeSystem or
 * ValueSet. Three columns: the code systems the package is authored against (local or external,
 * version, concept count); the selected code system's hierarchy, with search by code or words
 * run on the server a page at a time; and the open ValueSet, with its expansion, its change
 * against the published expansion (removals flagged) and the forms that bind it. With a draft
 * ValueSet open, a concept is added alone, added with everything under it (an `is-a` filter), or
 * excluded, and concepts already in it are ticked. Saving goes through the file editor's path,
 * with the draft's revision as `If-Match`.
 *
 * In a draft, a local CodeSystem of the package is edited here too (#524): the tree gains *Add
 * a code* and *+ under*, the concept panel becomes a form, and beside the tree the next version
 * and what the change reaches are shown, all worked out on the server. External code systems
 * stay read only.
 */
export function TerminologyBrowser({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  const file = useQuery({ ...fileQuery(platform.transport, id, path), retry: false })
  return (
    <section className={page}>
      <div>
        <Link to="/studio/$id" params={{ id }}>
          {intl.formatMessage(m.tbBack)}
        </Link>
      </div>
      {draft.data === undefined || file.data === undefined ? (
        draft.isError || file.isError ? (
          <p role="alert" className={alert}>
            {intl.formatMessage(m.tbFailed, { reason: reasonOf(intl, draft.error ?? file.error) })}
          </p>
        ) : (
          <p role="status">{intl.formatMessage(m.tbLoading)}</p>
        )
      ) : (
        <Browser
          key={`${file.data.revision}:${file.dataUpdatedAt}`}
          id={id}
          path={path}
          draft={draft.data}
          file={file.data}
          onReload={async () => {
            await Promise.all([draft.refetch(), file.refetch()])
          }}
        />
      )}
    </section>
  )
}

function Browser({ id, path, draft, file, onReload }: { id: string; path: string; draft: StudioPackagePage; file: StudioFile; onReload: () => Promise<void> }) {
  const intl = useIntl()
  const platform = usePlatform()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const first = opened(file)
  const systems = useQuery(codeSystemsQuery(platform.transport, id))
  // The ValueSet being edited, where it is saved, and the revision it was read at.
  const [valueSet, setValueSet] = useState<ValueSet | null>(first.kind === 'ValueSet' ? first.valueSet : null)
  const [target, setTarget] = useState(first.kind === 'ValueSet' ? path : null)
  const [revision, setRevision] = useState(file.revision)
  const [dirty, setDirty] = useState(false)
  const [system, setSystem] = useState<{ url: string; version: string | undefined } | null>(() => {
    if (first.kind === 'CodeSystem') return { url: first.url, version: first.version }
    const listed = first.kind === 'ValueSet' ? first.valueSet.compose?.include.find((r) => r.system !== undefined) : undefined
    return listed?.system === undefined ? null : { url: listed.system, version: listed.version }
  })
  const [selected, setSelected] = useState<string | null>(null)
  const editable = draft.editable && valueSet !== null
  const edit = (next: ValueSet) => {
    setValueSet(next)
    setDirty(true)
  }
  const save = useMutation({
    mutationFn: () => saveFile(platform.transport, id, target ?? path, `${JSON.stringify(valueSet, null, 2)}\n`, revision),
    onSuccess: async (next) => {
      setRevision(next.revision)
      setDirty(false)
      queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
      await queryClient.invalidateQueries({ queryKey: terminologyKey(id) })
      await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
      if (target !== null && target !== path) await navigate({ to: '/studio/$id/terminology/$', params: { id, _splat: target } })
    },
  })
  const stale = save.error instanceof StudioError && save.error.status === 412
  const chosen = systems.data?.find((s) => s.url === system?.url && (system.version === undefined || s.version === system.version))
  const title = valueSet === null ? (first.kind === 'CodeSystem' ? first.title : path) : (valueSet.title ?? valueSet.name ?? valueSet.url ?? path)
  const authoring: Authoring | null =
    draft.editable && chosen !== undefined && chosen.own && !chosen.external && chosen.path !== null && chosen.path !== undefined
      ? {
          path: chosen.path,
          revision,
          onSaved: (next) => setRevision(next.revision),
          onReload,
        }
      : null
  if (first.kind === 'other') {
    return <p className={muted}>{intl.formatMessage(m.tbNotTerminology)}</p>
  }
  return (
    <>
      <h1 className={heading}>{intl.formatMessage(m.tbHeading, { title })}</h1>
      <div className={columns}>
        <nav aria-label={intl.formatMessage(m.tbSystems)} className={panel}>
          <h2 className={subheading}>{intl.formatMessage(m.tbSystems)}</h2>
          {systems.data === undefined ? (
            systems.isError ? (
              <p role="alert" className={alert}>
                {intl.formatMessage(m.tbFailed, { reason: reasonOf(intl, systems.error) })}
              </p>
            ) : (
              <p role="status">{intl.formatMessage(m.tbLoading)}</p>
            )
          ) : systems.data.length === 0 ? (
            <p className={muted}>{intl.formatMessage(m.tbSystemsNone)}</p>
          ) : (
            <ul className={list}>
              {systems.data.map((s) => (
                <li key={`${s.url}|${s.version ?? ''}`}>
                  <button
                    type="button"
                    className={cx(small, css({ width: '100%' }))}
                    aria-pressed={chosen === s}
                    onClick={() => {
                      setSystem({ url: s.url, version: s.version ?? undefined })
                      setSelected(null)
                    }}
                  >
                    <span>{intl.formatMessage(m.tbSystemRow, { title: s.title ?? s.url, version: s.version ?? '', count: s.concepts })}</span>
                    <span className={cx(muted, css({ display: 'block' }))}>{intl.formatMessage(s.external ? m.tbExternal : m.tbLocal)}{s.own ? ` · ${intl.formatMessage(m.tbOwn)}` : ''}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </nav>
        <Concepts
          id={id}
          system={chosen}
          valueSet={valueSet}
          editable={editable}
          selected={selected}
          onSelect={setSelected}
          onEdit={edit}
          authoring={authoring}
        />
        <section aria-labelledby="tb-value-set" className={panel}>
          <h2 id="tb-value-set" className={subheading}>
            {intl.formatMessage(m.tbValueSet)}
          </h2>
          {valueSet === null ? (
            <NoValueSet
              id={id}
              draft={draft}
              sibling={first.kind === 'CodeSystem' ? first.url : undefined}
              onCreate={(created) => {
                setValueSet(created)
                setTarget(valueSetPath(created))
                setDirty(true)
              }}
            />
          ) : (
            <>
              <p className={minor}>{valueSet.version === undefined ? valueSet.url : `${valueSet.url ?? ''}|${valueSet.version}`}</p>
              {draft.editable ? null : <p className={muted}>{intl.formatMessage(m.tbReadOnly)}</p>}
              <Rules valueSet={valueSet} editable={editable} onEdit={edit} />
              <Expansion id={id} valueSet={valueSet} />
              {editable ? (
                <div className={css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center' })}>
                  <button type="button" className={button} disabled={!dirty || save.isPending || stale} onClick={() => save.mutate()}>
                    {intl.formatMessage(m.tbSave)}
                  </button>
                  {dirty ? <span className={muted}>{intl.formatMessage(m.tbUnsaved)}</span> : null}
                </div>
              ) : null}
              {save.isSuccess && !dirty ? <p role="status">{intl.formatMessage(m.tbSaved)}</p> : null}
              {stale ? (
                <div role="alert" className={css({ display: 'flex', flexDirection: 'column', gap: '2', alignItems: 'flex-start' })}>
                  <p className={alert}>{intl.formatMessage(m.tbStale)}</p>
                  <button type="button" className={button} onClick={() => void onReload()}>
                    {intl.formatMessage(m.tbReload)}
                  </button>
                </div>
              ) : save.isError ? (
                <p role="alert" className={alert}>
                  {intl.formatMessage(m.tbSaveFailed, { reason: reasonOf(intl, save.error) })}
                </p>
              ) : null}
            </>
          )}
        </section>
      </div>
    </>
  )
}

/** The codes of the open ValueSet's expansion, by `codeKey`, to tick in the tree. */
function useExpansion(id: string, valueSet: ValueSet | null) {
  const platform = usePlatform()
  return useQuery({
    queryKey: [...terminologyKey(id), 'expand', valueSet] as const,
    queryFn: ({ signal }) => (valueSet === null ? Promise.resolve(null) : expandValueSet(platform.transport, id, { valueSet }, { count: 1000 }, signal)),
    enabled: valueSet !== null,
    retry: false,
    placeholderData: (previous) => previous,
  })
}

function Concepts({
  id,
  system,
  valueSet,
  editable,
  selected,
  onSelect,
  onEdit,
  authoring,
}: {
  id: string
  system: StudioCodeSystem | undefined
  valueSet: ValueSet | null
  editable: boolean
  selected: string | null
  onSelect: (code: string | null) => void
  onEdit: (next: ValueSet) => void
  authoring: Authoring | null
}) {
  const intl = useIntl()
  const [adding, setAdding] = useState<{ parent: string | undefined } | null>(null)
  const searchId = useId()
  const hintId = useId()
  const [text, setText] = useState('')
  const [searched, setSearched] = useState('')
  const expansion = useExpansion(id, valueSet)
  const ticked = new Set((expansion.data?.contains ?? []).map((c) => codeKey(c.system, c.code)))
  const label = system === undefined ? '' : (system.title ?? system.url)
  return (
    <section aria-label={system === undefined ? intl.formatMessage(m.tbPickSystem) : intl.formatMessage(m.tbConcepts, { title: label })} className={panel}>
      {system === undefined ? (
        <p className={muted}>{intl.formatMessage(m.tbPickSystem)}</p>
      ) : (
        <>
          <h2 className={subheading}>{intl.formatMessage(m.tbConcepts, { title: label })}</h2>
          {authoring === null ? null : (
            <div>
              <button type="button" className={button} onClick={() => setAdding({ parent: undefined })}>
                {intl.formatMessage(m.ceAddCode)}
              </button>
            </div>
          )}
          <form
            role="search"
            className={css({ display: 'flex', flexWrap: 'wrap', gap: '2', alignItems: 'flex-end' })}
            onSubmit={(e) => {
              e.preventDefault()
              setSearched(text.trim())
            }}
          >
            <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', flex: '1', minWidth: '12rem' })}>
              <label htmlFor={searchId}>{intl.formatMessage(m.tbSearch)}</label>
              <input id={searchId} className={input} type="search" value={text} aria-describedby={hintId} onChange={(e) => setText(e.target.value)} />
              <span id={hintId} className={muted}>
                {intl.formatMessage(m.tbSearchHint)}
              </span>
            </div>
            <button type="submit" className={button}>
              {intl.formatMessage(m.tbSearchButton)}
            </button>
            {searched === '' ? null : (
              <button
                type="button"
                className={small}
                onClick={() => {
                  setText('')
                  setSearched('')
                }}
              >
                {intl.formatMessage(m.tbClearSearch)}
              </button>
            )}
          </form>
          <ConceptList
            key={`${system.url}|${searched}`}
            id={id}
            system={system}
            text={searched}
            parent={undefined}
            ticked={ticked}
            selected={selected}
            onSelect={(code) => {
              setAdding(null)
              onSelect(code)
            }}
            onAddUnder={authoring === null ? undefined : (code) => setAdding({ parent: code })}
          />
          {authoring !== null && adding !== null ? (
            <AddCode
              key={adding.parent ?? ''}
              id={id}
              authoring={authoring}
              parent={adding.parent}
              onAdded={(code) => {
                setAdding(null)
                onSelect(code)
              }}
              onCancel={() => setAdding(null)}
            />
          ) : selected === null ? null : (
            <Selected id={id} system={system} code={selected} valueSet={valueSet} editable={editable} onEdit={onEdit} authoring={authoring} onSelect={onSelect} />
          )}
          {authoring === null ? null : <CodeSystemChange id={id} path={authoring.path} />}
        </>
      )}
    </section>
  )
}

/** One page or more of concepts: under `parent`, or matching `text`. */
function ConceptList({
  id,
  system,
  text,
  parent,
  ticked,
  selected,
  onSelect,
  onAddUnder,
}: {
  id: string
  system: StudioCodeSystem
  text: string
  parent: string | undefined
  ticked: Set<string>
  selected: string | null
  onSelect: (code: string) => void
  onAddUnder: ((code: string) => void) | undefined
}) {
  const intl = useIntl()
  const platform = usePlatform()
  const [count, setCount] = useState(50)
  const ask = { system: system.url, version: system.version ?? undefined, ...(text === '' ? { parent } : { text }), count }
  const concepts = useQuery({ ...conceptsQuery(platform.transport, id, ask), placeholderData: (previous) => previous })
  if (concepts.data === undefined) {
    return concepts.isError ? (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.tbFailed, { reason: reasonOf(intl, concepts.error) })}
      </p>
    ) : (
      <p role="status">{intl.formatMessage(m.tbLoading)}</p>
    )
  }
  const data = concepts.data
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      {text === '' ? null : <p role="status">{data.total === 0 ? intl.formatMessage(m.tbNoneFound) : intl.formatMessage(m.tbFound, { total: data.total, shown: data.concepts.length })}</p>}
      <ul className={cx(list, parent === undefined ? undefined : nested)}>
        {data.concepts.map((concept) => (
          <ConceptItem key={concept.code} id={id} system={system} concept={concept} tree={text === ''} ticked={ticked} selected={selected} onSelect={onSelect} onAddUnder={onAddUnder} />
        ))}
      </ul>
      {data.concepts.length < data.total ? (
        <div>
          <button type="button" className={small} disabled={concepts.isFetching} onClick={() => setCount((n) => n + 50)}>
            {intl.formatMessage(m.tbMore)}
          </button>
        </div>
      ) : null}
    </div>
  )
}

function ConceptItem({
  id,
  system,
  concept,
  tree,
  ticked,
  selected,
  onSelect,
  onAddUnder,
}: {
  id: string
  system: StudioCodeSystem
  concept: StudioConceptRow
  tree: boolean
  ticked: Set<string>
  selected: string | null
  onSelect: (code: string) => void
  onAddUnder: ((code: string) => void) | undefined
}) {
  const intl = useIntl()
  const [open, setOpen] = useState(false)
  const included = ticked.has(codeKey(system.url, concept.code))
  return (
    <li>
      <div className={row}>
        {tree && concept.children > 0 ? (
          <button
            type="button"
            className={small}
            aria-expanded={open}
            aria-label={open ? intl.formatMessage(m.tbHideUnder, { code: concept.code }) : intl.formatMessage(m.tbShowUnder, { code: concept.code, count: concept.children })}
            onClick={() => setOpen((o) => !o)}
          >
            {open ? '−' : '+'}
          </button>
        ) : null}
        <button type="button" className={small} aria-pressed={selected === concept.code} onClick={() => onSelect(concept.code)}>
          {included ? <span aria-hidden="true">✓ </span> : null}
          <code>{concept.code}</code> {concept.display ?? ''}
          {included ? <span className={css({ srOnly: true })}> ({intl.formatMessage(m.tbIncluded)})</span> : null}
        </button>
        {included ? <span className={tag}>{intl.formatMessage(m.tbIncluded)}</span> : null}
        {concept.inactive ? <span className={tag}>{intl.formatMessage(m.tbInactive)}</span> : null}
        {onAddUnder === undefined ? null : (
          <button
            type="button"
            className={small}
            onClick={() => {
              setOpen(true)
              onAddUnder(concept.code)
            }}
          >
            {intl.formatMessage(m.ceAddUnder)}
            <span className={css({ srOnly: true })}> {concept.code}</span>
          </button>
        )}
      </div>
      {open ? <ConceptList id={id} system={system} text="" parent={concept.code} ticked={ticked} selected={selected} onSelect={onSelect} onAddUnder={onAddUnder} /> : null}
    </li>
  )
}

function Selected({
  id,
  system,
  code,
  valueSet,
  editable,
  onEdit,
  authoring,
  onSelect,
}: {
  id: string
  system: StudioCodeSystem
  code: string
  valueSet: ValueSet | null
  editable: boolean
  onEdit: (next: ValueSet) => void
  authoring: Authoring | null
  onSelect: (code: string | null) => void
}) {
  const intl = useIntl()
  const platform = usePlatform()
  const concept = useQuery({ ...conceptQuery(platform.transport, id, system.url, system.version ?? undefined, code), retry: false })
  const data = concept.data
  return (
    <section aria-labelledby="tb-selected" className={cx(panel, css({ bg: 'bg.canvas' }))}>
      <h3 id="tb-selected" className={minor}>
        {intl.formatMessage(m.tbSelected)}
      </h3>
      {data === undefined ? (
        concept.isError ? (
          <p role="alert" className={alert}>
            {intl.formatMessage(m.tbFailed, { reason: reasonOf(intl, concept.error) })}
          </p>
        ) : (
          <p role="status">{intl.formatMessage(m.tbLoading)}</p>
        )
      ) : (
        <>
          {authoring === null ? null : <EditCode key={`${data.code}:${concept.dataUpdatedAt}`} id={id} authoring={authoring} concept={data} onRenamed={onSelect} />}
          <p>
            <strong>{data.display ?? data.code}</strong>
          </p>
          <p className={muted}>{intl.formatMessage(m.tbConceptCode, { code: data.code, system: data.codeSystem })}</p>
          {data.inactive ? <p className={tag}>{intl.formatMessage(m.ceRetired)}</p> : null}
          <p className={muted}>{data.parents.length === 0 ? intl.formatMessage(m.tbTopLevel) : intl.formatMessage(m.tbParents, { parents: data.parents.join(', ') })}</p>
          {data.definition === null || data.definition === undefined ? null : (
            <div>
              <p className={minor}>{intl.formatMessage(m.tbDefinition)}</p>
              <p>{data.definition}</p>
            </div>
          )}
          <div>
            <p className={minor}>{intl.formatMessage(m.tbContainedIn)}</p>
            {data.valueSets.length === 0 ? (
              <p className={muted}>{intl.formatMessage(m.tbContainedInNone)}</p>
            ) : (
              <ul className={list}>
                {data.valueSets.map((v) => (
                  <li key={`${v.package}:${v.path}`}>
                    {v.title ?? v.url} <span className={muted}>({v.package})</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
          {editable && valueSet !== null ? (
            <div className={css({ display: 'flex', flexWrap: 'wrap', gap: '2' })}>
              <button type="button" className={small} onClick={() => onEdit(includeCode(valueSet, system.url, data.code, data.display ?? undefined))}>
                {intl.formatMessage(m.tbAdd, { code: data.code })}
              </button>
              <button type="button" className={small} disabled={!system.complete} onClick={() => onEdit(includeUnder(valueSet, system.url, data.code))}>
                {intl.formatMessage(m.tbAddUnder, { code: data.code })}
              </button>
              <button type="button" className={small} onClick={() => onEdit(excludeCode(valueSet, system.url, data.code, data.display ?? undefined))}>
                {intl.formatMessage(m.tbExclude, { code: data.code })}
              </button>
            </div>
          ) : null}
        </>
      )}
    </section>
  )
}

function Rules({ valueSet, editable, onEdit }: { valueSet: ValueSet; editable: boolean; onEdit: (next: ValueSet) => void }) {
  const intl = useIntl()
  const read = rules(valueSet)
  const describe = (rule: Rule) => {
    const system = rule.system ?? ''
    if (rule.under !== undefined) return intl.formatMessage(m.tbRuleUnder, { code: rule.under, system })
    if (rule.other) return intl.formatMessage(m.tbRuleOther, { system })
    return intl.formatMessage(rule.kind === 'include' ? m.tbRuleCodes : m.tbRuleExcluded, { system })
  }
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
      <h3 className={minor}>{intl.formatMessage(m.tbRules)}</h3>
      {read.length === 0 ? (
        <p className={muted}>{intl.formatMessage(m.tbRulesNone)}</p>
      ) : (
        <ul className={list}>
          {read.map((rule) => (
            <li key={`${rule.kind}:${rule.index}`} className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
              <div className={row}>
                <span>{describe(rule)}</span>
                {editable ? (
                  <button type="button" className={small} onClick={() => onEdit(removeRule(valueSet, rule.kind, rule.index))}>
                    {intl.formatMessage(m.tbRemoveRule)}
                  </button>
                ) : null}
              </div>
              {rule.codes.length === 0 ? null : (
                <ul className={cx(list, nested)}>
                  {rule.codes.map((c) => (
                    <li key={c.code} className={row}>
                      <span>
                        <code>{c.code}</code> {c.display ?? ''}
                      </span>
                      {editable ? (
                        <button type="button" className={small} aria-label={intl.formatMessage(m.tbRemoveCode, { code: c.code })} onClick={() => onEdit(removeListed(valueSet, rule.kind, rule.index, c.code))}>
                          ×
                        </button>
                      ) : null}
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

function Codes({ codes, className }: { codes: StudioExpansion['contains']; className?: string }) {
  return (
    <ul className={cx(list, className)}>
      {codes.map((c) => (
        <li key={codeKey(c.system, c.code)}>
          <code>{c.code}</code> {c.display ?? ''}
        </li>
      ))}
    </ul>
  )
}

function Expansion({ id, valueSet }: { id: string; valueSet: ValueSet }) {
  const intl = useIntl()
  const expansion = useExpansion(id, valueSet)
  if (expansion.data === undefined || expansion.data === null) {
    return expansion.isError ? (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.tbExpansionFailed, { reason: reasonOf(intl, expansion.error) })}
      </p>
    ) : (
      <p role="status">{intl.formatMessage(m.tbLoading)}</p>
    )
  }
  const data = expansion.data
  const change = data.change
  return (
    <>
      {expansion.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.tbExpansionFailed, { reason: reasonOf(intl, expansion.error) })}
        </p>
      ) : null}
      <section aria-labelledby="tb-expansion" className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <h3 id="tb-expansion" className={minor}>
          {intl.formatMessage(m.tbExpansion, { total: data.total })}
        </h3>
        <Codes codes={data.contains} />
        {data.contains.length < data.total ? <p className={muted}>{intl.formatMessage(m.tbExpansionShown, { shown: data.contains.length })}</p> : null}
      </section>
      {change === null || change === undefined ? null : (
        <section aria-labelledby="tb-change" className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          <h3 id="tb-change" className={minor}>
            {intl.formatMessage(m.tbChange)}
          </h3>
          <p className={muted}>{change.published === null || change.published === undefined ? intl.formatMessage(m.tbChangeFirst) : intl.formatMessage(m.tbChangeAgainst, { published: change.published })}</p>
          {change.addedTotal === 0 && change.removedTotal === 0 ? <p>{intl.formatMessage(m.tbChangeNone)}</p> : null}
          {change.addedTotal === 0 ? null : (
            <div>
              <p>{intl.formatMessage(m.tbAdded, { total: change.addedTotal })}</p>
              <Codes codes={change.added} />
            </div>
          )}
          {change.removedTotal === 0 ? null : (
            <div role="group" aria-label={intl.formatMessage(m.tbRemoved, { total: change.removedTotal })} className={removed}>
              <p>
                <span aria-hidden="true">⚠ </span>
                {intl.formatMessage(m.tbRemoved, { total: change.removedTotal })}
              </p>
              <Codes codes={change.removed} />
              <p className={muted}>{intl.formatMessage(m.tbRemovedFlag)}</p>
            </div>
          )}
        </section>
      )}
      <section aria-labelledby="tb-bound" className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        <h3 id="tb-bound" className={minor}>
          {intl.formatMessage(m.tbBoundBy)}
        </h3>
        {data.boundBy.length === 0 ? (
          <p className={muted}>{intl.formatMessage(m.tbBoundByNone)}</p>
        ) : (
          <ul className={list}>
            {data.boundBy.map((b) => (
              <li key={`${b.package}:${b.path}:${b.linkId}`}>{intl.formatMessage(m.tbBinding, { form: b.title ?? b.questionnaire ?? b.path, linkId: b.linkId, package: b.package })}</li>
            ))}
          </ul>
        )}
      </section>
    </>
  )
}

/** With no ValueSet open: the package's ValueSets to open, and in a draft, a new one. */
function NoValueSet({ id, draft, sibling, onCreate }: { id: string; draft: StudioPackagePage; sibling: string | undefined; onCreate: (created: ValueSet) => void }) {
  const intl = useIntl()
  const nameId = useId()
  const nameHintId = useId()
  const titleId = useId()
  const [name, setName] = useState('')
  const [title, setTitle] = useState('')
  const own = draft.files.filter((f) => f.resourceType === 'ValueSet')
  const taken = (path: string) => draft.files.some((f) => f.path === path)
  const created = newValueSet(name, title, draft.package.version, sibling ?? draft.files.find((f) => f.url !== null && f.url !== undefined && f.resourceType !== 'Library')?.url ?? undefined)
  return (
    <>
      <p className={muted}>{intl.formatMessage(m.tbNoValueSet)}</p>
      {own.length === 0 ? null : (
        <div>
          <h3 className={minor}>{intl.formatMessage(m.tbOpenValueSet)}</h3>
          <ul className={list}>
            {own.map((f) => (
              <li key={f.path}>
                <Link to="/studio/$id/terminology/$" params={{ id, _splat: f.path }}>
                  {f.path}
                </Link>
              </li>
            ))}
          </ul>
        </div>
      )}
      {draft.editable ? (
        <form
          className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}
          onSubmit={(e) => {
            e.preventDefault()
            if (created.id !== '' && !taken(valueSetPath(created))) onCreate(created)
          }}
        >
          <h3 className={minor}>{intl.formatMessage(m.tbNewValueSet)}</h3>
          <label htmlFor={nameId}>{intl.formatMessage(m.tbNewName)}</label>
          <input id={nameId} className={input} value={name} aria-describedby={nameHintId} onChange={(e) => setName(e.target.value)} />
          <span id={nameHintId} className={muted}>
            {intl.formatMessage(m.tbNewNameHint)}
          </span>
          <label htmlFor={titleId}>{intl.formatMessage(m.tbNewTitle)}</label>
          <input id={titleId} className={input} value={title} onChange={(e) => setTitle(e.target.value)} />
          <div>
            <button type="submit" className={button} disabled={created.id === '' || taken(valueSetPath(created))}>
              {intl.formatMessage(m.tbCreate)}
            </button>
          </div>
        </form>
      ) : null}
    </>
  )
}
