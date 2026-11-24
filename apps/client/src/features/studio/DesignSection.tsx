import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { useId, useMemo, useState } from 'react'
import { useIntl } from 'react-intl'

import { designMessages as dm, studioMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button, control, field } from '../reception/styles'
import { StudioError, packageQuery, type StudioPackageKind } from './api'
import { createDesign, createTable, type Created, type DraftPlan } from './create'
import {
  changedKeys,
  designsQuery,
  editorOf,
  filterDesigns,
  icu,
  inSection,
  noFilter,
  packageChoices,
  packageOf,
  sectionOf,
  statusOf,
  type DesignFilter,
  type DesignStatus,
  type NewKind,
  type SectionId,
  type StudioDesign,
  type StudioDesigns,
} from './designs'
import { heading, muted, page } from './StudioHome'
import { refusalText } from './wording'

const filters = css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'flex-end' })
const toolbar = css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center', justifyContent: 'space-between' })
const panel = css({ display: 'flex', flexDirection: 'column', gap: '3', p: '4', borderRadius: 'lg', border: '1px solid', borderColor: 'border', bg: 'bg.surface' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const secondary = css({ minHeight: 'control', px: 'controlX', borderRadius: 'md', border: '1px solid', borderColor: 'border', fontSize: 'control', cursor: 'pointer', _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' } })
const alert = css({ color: 'danger.text' })
const statuses: DesignStatus[] = ['published', 'yours', 'theirs', 'submitted']

/** Opens a design in its editor, in package version `id`. */
export function useOpenDesign() {
  const navigate = useNavigate()
  return (design: Pick<StudioDesign, 'kind' | 'path' | 'table'>, id: string) => {
    const target = editorOf(design, id)
    switch (target.to) {
      case '/studio/$id/forms/$':
        return navigate({ to: target.to, params: target.params })
      case '/studio/$id/workflows/$':
        return navigate({ to: target.to, params: target.params, search: target.search })
      case '/studio/$id/terminology/$':
        return navigate({ to: target.to, params: target.params })
      case '/studio/$id/programs/$':
        return navigate({ to: target.to, params: target.params })
      case '/studio/$id/age-bands/$':
        return navigate({ to: target.to, params: target.params })
      case '/studio/$id/indicators/$':
        return navigate({ to: target.to, params: target.params })
      case '/studio/$id/reports/$':
        return navigate({ to: target.to, params: target.params })
      case '/studio/$id/dashboards/$':
        return navigate({ to: target.to, params: target.params })
      case '/studio/$id/population/$':
        return navigate({ to: target.to, params: target.params })
    }
  }
}

/**
 * One section of the Studio (studio.md § The Studio home; #855): every design of its kinds across
 * packages, each in the version it opens in (a package's open draft, else its latest published
 * version), with its package, where it stands and its package's last change. Search and filters
 * narrow it, and a row opens its editor. *New* asks only which package and a title.
 */
export function DesignSection({ section, startNew }: { section: SectionId; startNew: boolean }) {
  const intl = useIntl()
  const platform = usePlatform()
  const designs = useQuery(designsQuery(platform.transport))
  const [creating, setCreating] = useState(startNew)
  const s = sectionOf(section)
  const name = intl.formatMessage(dm.dhSection, { section: icu(section) })
  return (
    <section className={page} aria-labelledby="studio-section">
      <div>
        <Link to="/studio">{intl.formatMessage(dm.dhHome)}</Link>
      </div>
      <div className={toolbar}>
        <div>
          <h1 id="studio-section" className={heading}>
            {name}
          </h1>
          <p className={muted}>{intl.formatMessage(dm.dhSectionHint, { section: icu(section) })}</p>
        </div>
        {s.creates && designs.data !== undefined && !creating ? (
          <button type="button" className={button} onClick={() => setCreating(true)}>
            {intl.formatMessage(dm.dhNew, { section: icu(section) })}
          </button>
        ) : null}
      </div>
      {designs.data === undefined ? (
        designs.isPending ? (
          <p role="status">{intl.formatMessage(dm.dhLoading)}</p>
        ) : (
          <p role="alert" className={alert}>
            {intl.formatMessage(designs.error instanceof StudioError && designs.error.status === 403 ? m.sdRefused : m.sdFailed)}
          </p>
        )
      ) : (
        <>
          {creating ? <NewDesign section={section} designs={designs.data} onCancel={() => setCreating(false)} /> : null}
          <Listing section={section} designs={designs.data} />
        </>
      )}
    </section>
  )
}

function Listing({ section, designs }: { section: SectionId; designs: StudioDesigns }) {
  const intl = useIntl()
  const open = useOpenDesign()
  const ids = { text: useId(), package: useId(), status: useId() }
  const [filter, setFilter] = useState<DesignFilter>(noFilter)
  const [view, setView] = useState<DataTableView>(emptyView)
  const all = inSection(designs.designs, section)
  const shown = filterDesigns(designs, all, filter)
  const name = intl.formatMessage(dm.dhSection, { section: icu(section) })
  const packages = [...new Set(all.map((d) => d.package))]
  const several = sectionOf(section).kinds.length > 1
  const status = (d: StudioDesign) => {
    const pkg = packageOf(designs, d.package)
    return intl.formatMessage(dm.dhStatus, { status: statusOf(pkg), holder: pkg?.draft?.holder ?? '' })
  }
  const changed = (d: StudioDesign) => {
    const at = packageOf(designs, d.package)?.changed
    return at === null || at === undefined ? intl.formatMessage(dm.dhNever) : intl.formatDate(at, { dateStyle: 'medium', timeStyle: 'short' })
  }
  const columns: DataTableColumn<StudioDesign>[] = [
    { id: 'title', header: intl.formatMessage(dm.dhColTitle), cell: (d) => d.title, sortValue: (d) => d.title.toLowerCase() },
    ...(several ? [{ id: 'type', header: intl.formatMessage(dm.dhColType), cell: (d: StudioDesign) => intl.formatMessage(dm.dhKind, { kind: icu(d.kind) }), sortValue: (d: StudioDesign) => d.kind }] : []),
    ...(section === 'decision-tables' ? [{ id: 'workflow', header: intl.formatMessage(dm.dhSection, { section: 'workflows' }), cell: (d: StudioDesign) => d.path }] : []),
    {
      id: 'package',
      header: intl.formatMessage(dm.dhColPackage),
      cell: (d) => `${packageOf(designs, d.package)?.title ?? d.package} · ${d.packageId}`,
      sortValue: (d) => d.package,
    },
    { id: 'status', header: intl.formatMessage(dm.dhColStatus), cell: status, sortValue: (d) => statusOf(packageOf(designs, d.package)) },
    { id: 'changed', header: intl.formatMessage(dm.dhColChanged), cell: changed, sortValue: (d) => packageOf(designs, d.package)?.changed ?? '' },
  ]
  if (all.length === 0) return <p role="status">{intl.formatMessage(dm.dhEmpty, { section: icu(section) })}</p>
  return (
    <>
      <div className={filters} role="search">
        <div className={field}>
          <label htmlFor={ids.text}>{intl.formatMessage(dm.dhSearch)}</label>
          <input id={ids.text} type="search" className={control} value={filter.text} onChange={(e) => setFilter({ ...filter, text: e.target.value })} />
        </div>
        <div className={field}>
          <label htmlFor={ids.package}>{intl.formatMessage(dm.dhFilterPackage)}</label>
          <select id={ids.package} className={control} value={filter.package} onChange={(e) => setFilter({ ...filter, package: e.target.value })}>
            <option value="">{intl.formatMessage(dm.dhPackageAll)}</option>
            {packages.map((p) => (
              <option key={p} value={p}>
                {packageOf(designs, p)?.title ?? p}
              </option>
            ))}
          </select>
        </div>
        <div className={field}>
          <label htmlFor={ids.status}>{intl.formatMessage(dm.dhFilterStatus)}</label>
          <select id={ids.status} className={control} value={filter.status} onChange={(e) => setFilter({ ...filter, status: e.target.value as DesignStatus | '' })}>
            <option value="">{intl.formatMessage(dm.dhStatusAny)}</option>
            {statuses.map((st) => (
              <option key={st} value={st}>
                {intl.formatMessage(dm.dhStatus, { status: st, holder: intl.formatMessage(dm.dhSomeone) })}
              </option>
            ))}
          </select>
        </div>
      </div>
      <p className={muted}>{intl.formatMessage(dm.dhCount, { section: icu(section), count: shown.length })}</p>
      {shown.length === 0 ? (
        <p role="status">{intl.formatMessage(dm.dhNoMatch)}</p>
      ) : (
        <>
          <p className={muted}>{intl.formatMessage(dm.dhTableHint)}</p>
          <DataTable caption={name} columns={columns} data={shown} getRowId={(d) => `${d.packageId}/${d.path}#${d.table ?? ''}`} view={view} onViewChange={setView} onRowActivate={(d) => void open(d, d.packageId)} />
        </>
      )}
    </>
  )
}

const kinds: StudioPackageKind[] = ['program', 'national-core', 'terminology', 'country-configuration', 'override', 'language', 'reporting', 'population']

/** *New* in a section: which package (a draft the person holds, a published package whose next
 * version they start, or a new package), then a title; a decision table also asks which workflow
 * it goes in. It is created through the file editor's path and opens in its editor. */
function NewDesign({ section, designs, onCancel }: { section: SectionId; designs: StudioDesigns; onCancel: () => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const open = useOpenDesign()
  const ids = { title: useId(), name: useId(), packageTitle: useId(), kind: useId(), canonical: useId(), what: useId(), workflow: useId(), heading: useId() }
  const tables = section === 'decision-tables'
  const workflows = designs.designs.filter((d) => d.kind === 'workflow')
  const reports = section === 'reports' || section === 'dashboards'
  const population = section === 'population'
  // A Report or a standard Dashboard belongs in a Reporting package (ADR 0070, ADR 0075); a
  // year's estimates in a Population package that has none yet (ADR 0071).
  const choices = packageChoices(designs).filter(
    (c) =>
      (!tables || workflows.some((w) => w.package === c.name)) &&
      (!reports || packageOf(designs, c.name)?.kind === 'reporting') &&
      (!population || (packageOf(designs, c.name)?.kind === 'population' && !designs.designs.some((d) => d.kind === 'population' && d.package === c.name))),
  )
  const [chosen, setChosen] = useState<string>(choices[0]?.name ?? (tables ? '' : '*new'))
  const [title, setTitle] = useState('')
  const [what, setWhat] = useState<NewKind>(section === 'concepts' ? 'code-system' : section === 'workflows' ? 'workflow' : section === 'age-bands' ? 'age-band-set' : section === 'indicators' ? 'indicator' : section === 'dashboards' ? 'dashboard' : reports ? 'report' : population ? 'population' : 'form')
  const [blank, setBlank] = useState({ name: '', title: '', kind: (reports ? 'reporting' : population ? 'population' : 'program') as StudioPackageKind, canonical: '' })
  const inPackage = useMemo(() => workflows.filter((w) => w.package === chosen), [workflows, chosen])
  const [workflow, setWorkflow] = useState('')
  const workflowPath = inPackage.some((w) => w.path === workflow) ? workflow : (inPackage[0]?.path ?? '')
  const create = useMutation({
    mutationFn: async (): Promise<Created> => {
      const choice = choices.find((c) => c.name === chosen)
      const plan: DraftPlan | undefined = chosen === '*new' ? { kind: 'blank', draft: { ...blank, version: '0.1.0' } } : choice?.plan
      if (plan === undefined) throw new Error(intl.formatMessage(dm.dhNewNoPackage))
      if (tables) return createTable(platform.transport, plan, workflowPath, title)
      return createDesign(platform.transport, plan, what, title, chosen === '*new' ? blank.canonical : (packageOf(designs, chosen)?.canonical ?? null))
    },
    onSuccess: async (created) => {
      queryClient.setQueryData(packageQuery(platform.transport, created.page.package.id).queryKey, created.page)
      await Promise.all(changedKeys.map((queryKey) => queryClient.invalidateQueries({ queryKey, exact: true })))
      const kind = tables ? 'decision-table' : what
      await open({ kind, path: created.path, table: created.table ?? null }, created.page.package.id)
    },
  })
  const failure =
    create.error instanceof StudioError
      ? create.error.refusal === null || create.error.refusal === undefined
        ? create.error.detail
        : refusalText(intl, create.error.refusal)
      : create.error instanceof Error
        ? create.error.message
        : ''
  if (tables && choices.length === 0)
    return (
      <div className={panel}>
        <p role="status">{intl.formatMessage(dm.dhNewNoWorkflow)}</p>
        <div>
          <button type="button" className={secondary} onClick={onCancel}>
            {intl.formatMessage(dm.dhNewCancel)}
          </button>
        </div>
      </div>
    )
  return (
    <form
      className={panel}
      aria-labelledby={ids.heading}
      onSubmit={(e) => {
        e.preventDefault()
        create.mutate()
      }}
    >
      <h2 id={ids.heading} className={subheading}>
        {intl.formatMessage(dm.dhNewHeading, { section: icu(section) })}
      </h2>
      <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '1', border: 'none', p: '0', m: '0' })}>
        <legend className={css({ fontWeight: 'medium', mb: '1' })}>{intl.formatMessage(dm.dhNewPackage)}</legend>
        {choices.map((c) => (
          <label key={c.name} className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
            <input type="radio" name="package" value={c.name} checked={chosen === c.name} onChange={() => setChosen(c.name)} />
            {c.plan.kind === 'reuse' ? intl.formatMessage(dm.dhNewInDraft, { title: c.title, id: c.plan.id }) : intl.formatMessage(dm.dhNewNextVersion, { title: c.title, id: c.plan.copyOf })}
          </label>
        ))}
        {tables ? null : (
          <label className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
            <input type="radio" name="package" value="*new" checked={chosen === '*new'} onChange={() => setChosen('*new')} />
            {intl.formatMessage(dm.dhNewBlank)}
          </label>
        )}
      </fieldset>
      {chosen === '*new' ? (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3', pl: '6' })}>
          <div className={field}>
            <label htmlFor={ids.name}>{intl.formatMessage(dm.dhNewBlankName)}</label>
            <input id={ids.name} className={control} required value={blank.name} onChange={(e) => setBlank({ ...blank, name: e.target.value })} />
          </div>
          <div className={field}>
            <label htmlFor={ids.packageTitle}>{intl.formatMessage(dm.dhNewBlankTitle)}</label>
            <input id={ids.packageTitle} className={control} required value={blank.title} onChange={(e) => setBlank({ ...blank, title: e.target.value })} />
          </div>
          <div className={field}>
            <label htmlFor={ids.kind}>{intl.formatMessage(dm.dhNewBlankKind)}</label>
            <select id={ids.kind} className={control} value={blank.kind} onChange={(e) => setBlank({ ...blank, kind: e.target.value as StudioPackageKind })}>
              {kinds.map((kind) => (
                <option key={kind} value={kind}>
                  {kind}
                </option>
              ))}
            </select>
          </div>
          <div className={field}>
            <label htmlFor={ids.canonical}>{intl.formatMessage(dm.dhNewBlankCanonical)}</label>
            <input id={ids.canonical} className={control} required type="url" value={blank.canonical} onChange={(e) => setBlank({ ...blank, canonical: e.target.value })} />
          </div>
        </div>
      ) : null}
      {section === 'concepts' ? (
        <div className={field}>
          <label htmlFor={ids.what}>{intl.formatMessage(dm.dhNewWhat)}</label>
          <select id={ids.what} className={control} value={what} onChange={(e) => setWhat(e.target.value as NewKind)}>
            <option value="code-system">{intl.formatMessage(dm.dhKind, { kind: 'codeSystem' })}</option>
            <option value="value-set">{intl.formatMessage(dm.dhKind, { kind: 'valueSet' })}</option>
          </select>
        </div>
      ) : null}
      {tables ? (
        <div className={field}>
          <label htmlFor={ids.workflow}>{intl.formatMessage(dm.dhNewWorkflow)}</label>
          <select id={ids.workflow} className={control} value={workflowPath} onChange={(e) => setWorkflow(e.target.value)}>
            {inPackage.map((w) => (
              <option key={w.path} value={w.path}>
                {w.title}
              </option>
            ))}
          </select>
        </div>
      ) : null}
      <div className={field}>
        <label htmlFor={ids.title}>{intl.formatMessage(dm.dhNewTitle)}</label>
        <input id={ids.title} className={control} required value={title} onChange={(e) => setTitle(e.target.value)} />
      </div>
      <div className={css({ display: 'flex', gap: '3' })}>
        <button type="submit" className={button} disabled={create.isPending}>
          {intl.formatMessage(dm.dhNewCreate)}
        </button>
        <button type="button" className={secondary} onClick={onCancel}>
          {intl.formatMessage(dm.dhNewCancel)}
        </button>
      </div>
      {create.isError ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(dm.dhNewFailed, { reason: failure })}
        </p>
      ) : null}
    </form>
  )
}
