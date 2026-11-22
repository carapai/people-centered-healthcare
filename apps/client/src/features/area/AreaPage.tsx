import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { useIntl } from 'react-intl'

import { areaMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { AreaError, type PlaceLevel, areaQuery, facilitiesByDistrict } from './api'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const table = css({ borderCollapse: 'collapse', '& th, & td': { textAlign: 'start', px: '2', py: '1', borderBottom: '1px solid', borderColor: 'border' } })

/**
 * The session's place and its area (ADR 0094): at a district, region or the national place, the
 * facilities whose summaries its dashboards and Reports roll up over; at a facility, the facility
 * itself. An area session reads summaries, never patient-level records.
 */
export function AreaPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const area = useQuery(areaQuery(transport))
  const levelName = (level: PlaceLevel) => intl.formatMessage(m[`arLevel_${level}`])
  if (area.data === undefined) {
    if (area.isPending) return <p role="status">{intl.formatMessage(m.arLoading)}</p>
    const missing = area.error instanceof AreaError && area.error.status === 404
    return (
      <section className={page}>
        <h1 className={heading}>{intl.formatMessage(m.arTitle)}</h1>
        <p role="alert" className={css({ color: missing ? 'fg.muted' : 'danger.text' })}>
          {intl.formatMessage(missing ? m.arNotHere : m.arFailed)}
        </p>
      </section>
    )
  }
  const data = area.data
  const groups = facilitiesByDistrict(data)
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.arTitle)}</h1>
      <p>{intl.formatMessage(m.arPlace, { name: data.place?.name ?? data.code, level: levelName(data.level) })}</p>
      {data.level === 'facility' ? null : <p>{intl.formatMessage(m.arAreaIntro, { count: data.facilities.length })}</p>}
      {groups.map(({ district, facilities }) => (
        <section key={district?.code ?? data.code} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
          {data.level === 'district' ? null : <h2 className={subheading}>{district?.name ?? data.code}</h2>}
          <table className={table}>
            <thead>
              <tr>
                <th scope="col">{intl.formatMessage(m.arCode)}</th>
                <th scope="col">{intl.formatMessage(m.arName)}</th>
                <th scope="col">{intl.formatMessage(m.arCareLevel)}</th>
                <th scope="col">{intl.formatMessage(m.arStatus)}</th>
              </tr>
            </thead>
            <tbody>
              {facilities.map((facility) => (
                <tr key={facility.code}>
                  <td>{facility.code}</td>
                  <td>{facility.name}</td>
                  <td>{facility.care_level ?? ''}</td>
                  <td>{intl.formatMessage(facility.active ? m.arOpen : m.arClosed)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ))}
    </section>
  )
}
