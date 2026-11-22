import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useId, type ReactNode } from 'react'
import { useIntl } from 'react-intl'

import { designMessages as dm, studioMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { StudioError } from './api'
import { designsQuery, icu, inSection, sections, type SectionId } from './designs'

export const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '6xl' })
export const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
export const muted = css({ color: 'fg.muted' })
const grid = css({ display: 'grid', gap: '4', gridTemplateColumns: { base: '1fr', md: 'repeat(2, minmax(0, 1fr))', lg: 'repeat(3, minmax(0, 1fr))' }, listStyle: 'none', p: '0', m: '0' })
const card = css({ display: 'flex', flexDirection: 'column', gap: '2', p: '4', borderRadius: 'lg', border: '1px solid', borderColor: 'border', bg: 'bg.surface', height: '100%' })
const soon = css({ borderStyle: 'dashed', bg: 'transparent' })
const cardTitle = css({ fontSize: 'lg', fontWeight: 'semibold' })
const count = css({ fontSize: 'sm', fontWeight: 'medium' })
const actions = css({ display: 'flex', flexWrap: 'wrap', gap: '3', mt: 'auto', pt: '2' })
const badge = css({ alignSelf: 'flex-start', fontSize: 'xs', fontWeight: 'semibold', px: '2', py: '0.5', borderRadius: 'full', border: '1px solid', borderColor: 'border', color: 'fg.muted' })

/** A link to `/studio/<section>`; New opens its panel. */
function SectionLink({ section, isNew, children }: { section: SectionId; isNew?: boolean; children: ReactNode }) {
  const search = isNew === true ? { new: true } : {}
  switch (section) {
    case 'forms':
      return <Link to="/studio/forms" search={search}>{children}</Link>
    case 'workflows':
      return <Link to="/studio/workflows" search={search}>{children}</Link>
    case 'concepts':
      return <Link to="/studio/concepts" search={search}>{children}</Link>
    case 'decision-tables':
      return <Link to="/studio/decision-tables" search={search}>{children}</Link>
    case 'programs':
      return <Link to="/studio/programs" search={search}>{children}</Link>
    case 'age-bands':
      return <Link to="/studio/age-bands" search={search}>{children}</Link>
    case 'indicators':
      return <Link to="/studio/indicators" search={search}>{children}</Link>
    case 'reports':
      return <Link to="/studio/reports" search={search}>{children}</Link>
  }
}

export { SectionLink }

/**
 * The Studio's home (studio.md § The Studio home; #855): what a person designs, one card per
 * kind with how many there are, *Open* and *New*. Reports (#556) open in the Report designer
 * (#559); dashboards are shown, marked coming soon. The packages, for checking, review, approval and
 * publication, are one more card.
 */
export function StudioHome() {
  const intl = useIntl()
  const platform = usePlatform()
  const designs = useQuery(designsQuery(platform.transport))
  const data = designs.data
  return (
    <section className={page} aria-labelledby="studio-home">
      <h1 id="studio-home" className={heading}>
        {intl.formatMessage(m.sdTitle)}
      </h1>
      <p className={muted}>{intl.formatMessage(dm.dhIntro)}</p>
      {data === undefined ? (
        designs.isPending ? (
          <p role="status">{intl.formatMessage(dm.dhLoading)}</p>
        ) : (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(designs.error instanceof StudioError && designs.error.status === 403 ? m.sdRefused : m.sdFailed)}
          </p>
        )
      ) : (
        <ul className={grid}>
          {sections.map((s) => {
            const n = inSection(data.designs, s.id).length
            return (
              <Card key={s.id} title={intl.formatMessage(dm.dhSection, { section: icu(s.id) })} hint={intl.formatMessage(dm.dhSectionHint, { section: icu(s.id) })}>
                <p className={count}>{intl.formatMessage(dm.dhCount, { section: icu(s.id), count: n })}</p>
                <div className={actions}>
                  <SectionLink section={s.id}>{intl.formatMessage(dm.dhOpen, { section: icu(s.id) })}</SectionLink>
                  {s.creates ? (
                    <SectionLink section={s.id} isNew>
                      {intl.formatMessage(dm.dhNew, { section: icu(s.id) })}
                    </SectionLink>
                  ) : null}
                </div>
              </Card>
            )
          })}
          <Card title={intl.formatMessage(dm.dhDashboards)} hint={intl.formatMessage(dm.dhDashboardsHint)} soon>
            <span className={badge}>{intl.formatMessage(dm.dhSoon)}</span>
            <p className={muted}>{intl.formatMessage(dm.dhSoonHint)}</p>
          </Card>
          <Card title={intl.formatMessage(dm.dhPackages)} hint={intl.formatMessage(dm.dhPackagesHint)}>
            <p className={count}>{intl.formatMessage(dm.dhPackagesCount, { count: data.packages.length })}</p>
            <div className={actions}>
              <Link to="/studio/packages">{intl.formatMessage(dm.dhPackagesOpen)}</Link>
            </div>
          </Card>
        </ul>
      )}
    </section>
  )
}

function Card({ title, hint, soon: isSoon, children }: { title: string; hint: string; soon?: boolean; children: ReactNode }) {
  const id = useId()
  return (
    <li>
      <article aria-labelledby={id} className={isSoon === true ? `${card} ${soon}` : card}>
        <h2 id={id} className={cardTitle}>
          {title}
        </h2>
        <p className={muted}>{hint}</p>
        {children}
      </article>
    </li>
  )
}
