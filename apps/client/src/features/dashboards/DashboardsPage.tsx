import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { explorerMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { type Explorer, ExplorerError, type PlaceLevel, explorerQuery, forViewer } from '../explorer/api'
import { LiveVisualization } from '../explorer/ExplorerPage'
import { DASHBOARDS_KEY, type Dashboard, DashboardsError, type SavedVisualization, type Share, changeDashboard, createDashboard, dashboardsQuery, explorerSearch, sendable, splitList } from './api'

const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const layout = css({ display: 'grid', gap: '4', gridTemplateColumns: { base: '1fr', md: '16rem 1fr' }, alignItems: 'start' })
const panel = css({ display: 'flex', flexDirection: 'column', gap: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md', p: '3' })
const field = css({ border: '1px solid', borderColor: 'border.control', borderRadius: 'md', px: '2', minHeight: 'control', bg: 'bg.surface' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const group = css({ display: 'flex', flexDirection: 'column', gap: '1', border: 'none', p: '0', m: '0', '& legend': { fontWeight: 'semibold', mb: '1' } })
const pressable = css({
  border: '1px solid',
  borderColor: 'border.control',
  borderRadius: 'md',
  px: 'controlX',
  minHeight: 'control',
  bg: 'bg.surface',
  cursor: 'pointer',
  textAlign: 'start',
  '&[aria-pressed=true]': { bg: 'bg.selected', color: 'fg.selected', borderColor: 'accent.default', fontWeight: 'semibold' },
})
const LEVELS: PlaceLevel[] = ['facility', 'district', 'region', 'national']
/** The pseudo-Dashboard of the session's own Visualizations on none of its Dashboards. */
const UNPLACED = '\u0000unplaced'

/**
 * The Dashboards tab (ADR 0075; analytics.md § Dashboards; #602): standard and saved Dashboards
 * on the left, the picked one's Visualizations on the right, each drawn for the viewer's own area
 * and with *Open in explorer*. Its owner shares it with people, roles and place levels. On a
 * Facility Edge Node (#879) it lists the active Reporting packages' standard Dashboards, offline,
 * over the facility's own figures.
 */
export function DashboardsPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const explorer = useQuery(explorerQuery(transport))
  const dashboards = useQuery(dashboardsQuery(transport))
  if (explorer.data === undefined || dashboards.data === undefined) {
    const error = explorer.error ?? dashboards.error
    if (error === null) return <p role="status">{intl.formatMessage(m.dbLoading)}</p>
    const forbidden = (error instanceof ExplorerError || error instanceof DashboardsError) && error.status === 403
    return (
      <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        <h1 className={heading}>{intl.formatMessage(m.dbTitle)}</h1>
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(forbidden ? m.dbForbidden : m.dbFailed)}
        </p>
      </section>
    )
  }
  return <Board explorer={explorer.data} dashboards={dashboards.data.dashboards} unplaced={dashboards.data.visualizations} canAuthor={dashboards.data.canAuthor} />
}

function Board({ explorer, dashboards, unplaced, canAuthor }: { explorer: Explorer; dashboards: Dashboard[]; unplaced: SavedVisualization[]; canAuthor: boolean }) {
  const intl = useIntl()
  const [picked, setPicked] = useState<string | undefined>(dashboards[0]?.id)
  const standard = dashboards.filter((d) => d.kind === 'standard')
  const saved = dashboards.filter((d) => d.kind === 'saved')
  const open = dashboards.find((d) => d.id === picked)
  const item = (d: Dashboard) => (
    <li key={d.id}>
      <button type="button" className={pressable} aria-pressed={picked === d.id} onClick={() => setPicked(d.id)}>
        {d.title}
        <span className={css({ display: 'block', fontSize: 'xs', fontWeight: 'normal' })}>
          {d.kind === 'standard' ? intl.formatMessage(m.dbStandardFrom, { package: d.package ?? '' }) : intl.formatMessage(d.mine ? m.dbMine : m.dbSharedWithYou)}
        </span>
      </button>
    </li>
  )
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h1 className={heading}>{intl.formatMessage(m.dbTitle)}</h1>
      <div className={layout}>
        <nav className={panel} aria-label={intl.formatMessage(m.dbList)}>
          <h2 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.dbStandard)}</h2>
          {standard.length === 0 ? <p className={muted}>{intl.formatMessage(m.dbNoStandard)}</p> : <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>{standard.map(item)}</ul>}
          <h2 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.dbSaved)}</h2>
          {/* On an edge (#879) only the standard Dashboards are here: saved ones are user content kept at central (#602). */}
          {explorer.where === 'facility' ? <p className={muted}>{intl.formatMessage(m.dbSavedAtCentral)}</p> : saved.length === 0 ? <p className={muted}>{intl.formatMessage(m.dbNoSaved)}</p> : <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>{saved.map(item)}</ul>}
          {unplaced.length > 0 ? (
            <button type="button" className={pressable} aria-pressed={picked === UNPLACED} onClick={() => setPicked(UNPLACED)}>
              {intl.formatMessage(m.dbUnplaced)}
            </button>
          ) : null}
          {canAuthor ? <NewDashboard onMade={setPicked} /> : null}
        </nav>
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
          {open !== undefined ? (
            <OpenDashboard key={open.id} explorer={explorer} dashboard={open} canAuthor={canAuthor} onCopied={setPicked} />
          ) : picked === UNPLACED ? (
            <Items explorer={explorer} items={unplaced} label={intl.formatMessage(m.dbUnplaced)} />
          ) : (
            <p className={muted}>{intl.formatMessage(m.dbPick)}</p>
          )}
        </div>
      </div>
    </section>
  )
}

function NewDashboard({ onMade }: { onMade: (id: string) => void }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const [title, setTitle] = useState('')
  const make = useMutation({
    mutationFn: () => createDashboard(transport, { title: title.trim(), shares: [] }),
    onSuccess: async (made) => {
      setTitle('')
      await queryClient.invalidateQueries({ queryKey: DASHBOARDS_KEY })
      onMade(made.id)
    },
  })
  return (
    <form
      className={css({ display: 'flex', flexDirection: 'column', gap: '2', borderTop: '1px solid', borderColor: 'border', pt: '3' })}
      aria-label={intl.formatMessage(m.dbNew)}
      onSubmit={(e) => {
        e.preventDefault()
        if (title.trim() !== '') make.mutate()
      }}
    >
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontWeight: 'semibold' })}>
        {intl.formatMessage(m.dbNew)}
        <input className={field} value={title} onChange={(e) => setTitle(e.target.value)} maxLength={120} placeholder={intl.formatMessage(m.dbNewTitle)} />
      </label>
      <button type="submit" className={pressable} disabled={title.trim() === '' || make.isPending}>
        {intl.formatMessage(m.dbCreate)}
      </button>
      {make.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {make.error instanceof DashboardsError && make.error.detail !== undefined ? make.error.detail : intl.formatMessage(m.dbChangeFailed)}
        </p>
      ) : null}
    </form>
  )
}

function OpenDashboard({ explorer, dashboard, canAuthor, onCopied }: { explorer: Explorer; dashboard: Dashboard; canAuthor: boolean; onCopied: (id: string) => void }) {
  return (
    <>
      {dashboard.kind === 'standard' ? <Standard dashboard={dashboard} canAuthor={canAuthor} onCopied={onCopied} /> : null}
      {dashboard.mine && canAuthor ? <Sharing dashboard={dashboard} /> : null}
      <Items explorer={explorer} items={dashboard.items} label={dashboard.title} />
    </>
  )
}

/**
 * A standard Dashboard (#603): where it comes from, that it changes only in a new version of its
 * Reporting package, and, for a user holding `analytics.author`, *Save as* their own copy.
 */
function Standard({ dashboard, canAuthor, onCopied }: { dashboard: Dashboard; canAuthor: boolean; onCopied: (id: string) => void }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const [title, setTitle] = useState(intl.formatMessage(m.dbSaveAsCopy, { title: dashboard.title }))
  const copy = useMutation({
    mutationFn: () => createDashboard(transport, { title: title.trim(), shares: [], copyOf: dashboard.id }),
    onSuccess: async (made) => {
      await queryClient.invalidateQueries({ queryKey: DASHBOARDS_KEY })
      onCopied(made.id)
    },
  })
  return (
    <div className={panel}>
      <p className={muted}>
        {dashboard.description === undefined ? null : `${dashboard.description} `}
        {intl.formatMessage(m.dbStandardNote)} {intl.formatMessage(m.dbStandardFrom, { package: dashboard.package ?? '' })}.
      </p>
      {canAuthor ? (
        <form
          className={css({ display: 'flex', gap: '2', alignItems: 'end', flexWrap: 'wrap' })}
          aria-label={intl.formatMessage(m.dbSaveAs)}
          onSubmit={(e) => {
            e.preventDefault()
            if (title.trim() !== '') copy.mutate()
          }}
        >
          <label className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontWeight: 'semibold' })}>
            {intl.formatMessage(m.dbSaveAsTitle)}
            <input className={field} value={title} onChange={(e) => setTitle(e.target.value)} maxLength={120} />
          </label>
          <button type="submit" className={pressable} disabled={title.trim() === '' || copy.isPending}>
            {intl.formatMessage(m.dbSaveAsButton)}
          </button>
          {copy.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {copy.error instanceof DashboardsError && copy.error.detail !== undefined ? copy.error.detail : intl.formatMessage(m.dbChangeFailed)}
            </p>
          ) : null}
        </form>
      ) : null}
    </div>
  )
}

/** A Dashboard's Visualizations, each drawn for the viewer's own area, with *Open in explorer*. */
function Items({ explorer, items, label }: { explorer: Explorer; items: SavedVisualization[]; label: string }) {
  const intl = useIntl()
  const headingId = useId()
  return (
    <section aria-labelledby={headingId} className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h2 id={headingId} className={css({ fontSize: 'xl', fontWeight: 'semibold' })}>
        {label}
      </h2>
      {items.length === 0 ? <p className={muted}>{intl.formatMessage(m.dbEmpty)}</p> : null}
      {items.map((item) => (
        <article key={item.id} className={panel} aria-label={item.title}>
          <div className={css({ display: 'flex', gap: '3', justifyContent: 'space-between', alignItems: 'baseline', flexWrap: 'wrap' })}>
            <h3 className={css({ fontWeight: 'semibold' })}>{item.title}</h3>
            <Link to="/explore" search={explorerSearch(item.visualization)}>
              {intl.formatMessage(m.dbOpenInExplorer)}
            </Link>
          </div>
          <LiveVisualization explorer={explorer} shown={forViewer(explorer, item.visualization)} />
        </article>
      ))}
    </section>
  )
}

/** Who the session's own Dashboard is shared with: place levels, roles and people, and its title. */
function Sharing({ dashboard }: { dashboard: Dashboard }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const shares = dashboard.shares ?? []
  const [levels, setLevels] = useState<PlaceLevel[]>(shares.flatMap((s) => (s.with === 'level' ? [s.level] : [])))
  const [roles, setRoles] = useState(shares.flatMap((s) => (s.with === 'role' ? [s.role] : [])).join(', '))
  const [people, setPeople] = useState(shares.flatMap((s) => (s.with === 'person' ? [s.username] : [])).join(', '))
  const change = useMutation({
    mutationFn: () => {
      const next: Share[] = [...levels.map((level) => ({ with: 'level', level }) as const), ...splitList(roles).map((role) => ({ with: 'role', role }) as const), ...splitList(people).map((username) => ({ with: 'person', username }) as const)]
      return changeDashboard(transport, dashboard.id, { title: dashboard.title, shares: next.map(sendable) })
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: DASHBOARDS_KEY }),
  })
  const summary = shares.map((s) => (s.with === 'level' ? intl.formatMessage(m[`dbLevel_${s.level}`]) : s.with === 'role' ? s.role : s.username)).join(', ')
  return (
    <form
      className={panel}
      aria-label={intl.formatMessage(m.dbSharing)}
      onSubmit={(e) => {
        e.preventDefault()
        change.mutate()
      }}
    >
      <h2 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.dbSharing)}</h2>
      <p className={muted}>
        {shares.length === 0 ? intl.formatMessage(m.dbNotShared) : intl.formatMessage(m.dbSharedWith, { shares: summary })} {intl.formatMessage(m.dbSharingIntro)}
      </p>
      <fieldset className={group}>
        <legend>{intl.formatMessage(m.dbLevels)}</legend>
        <div className={css({ display: 'flex', gap: '3', flexWrap: 'wrap' })}>
          {LEVELS.map((level) => (
            <label key={level} className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
              <input type="checkbox" checked={levels.includes(level)} onChange={() => setLevels(levels.includes(level) ? levels.filter((l) => l !== level) : [...levels, level])} />
              {intl.formatMessage(m[`dbLevel_${level}`])}
            </label>
          ))}
        </div>
      </fieldset>
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontWeight: 'semibold' })}>
        {intl.formatMessage(m.dbRoles)}
        <input className={field} value={roles} onChange={(e) => setRoles(e.target.value)} aria-describedby={`${dashboard.id}-hint`} />
      </label>
      <label className={css({ display: 'flex', flexDirection: 'column', gap: '1', fontWeight: 'semibold' })}>
        {intl.formatMessage(m.dbPeople)}
        <input className={field} value={people} onChange={(e) => setPeople(e.target.value)} aria-describedby={`${dashboard.id}-hint`} />
      </label>
      <p id={`${dashboard.id}-hint`} className={muted}>
        {intl.formatMessage(m.dbListHint)}
      </p>
      <div className={css({ display: 'flex', gap: '3', alignItems: 'center', flexWrap: 'wrap' })}>
        <button type="submit" className={pressable} disabled={change.isPending}>
          {intl.formatMessage(m.dbSaveSharing)}
        </button>
        {change.isSuccess ? <p role="status">{intl.formatMessage(m.dbSharingSaved)}</p> : null}
        {change.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {change.error instanceof DashboardsError && change.error.detail !== undefined ? change.error.detail : intl.formatMessage(m.dbChangeFailed)}
          </p>
        ) : null}
      </div>
    </form>
  )
}
