import { css } from '@health/design-system/css'
import { useQueries, useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { inboxMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import type { Transport } from '../../platform/types'
import { patientName } from '../reception/api'

/** An inbox item, as `/api/inbox` gives it. */
interface Item {
  kind: 'result' | 'documentation' | 'request'
  title: string
  patient: string
  /** The authoritative resource, `Type/id`. */
  reference: string
  priority: 'routine' | 'stat'
  since: string
}

async function inbox(transport: Transport): Promise<Item[]> {
  const response = await transport.request({ method: 'GET', path: '/inbox' })
  if (response.status !== 200) throw new Error(`inbox: ${response.status}`)
  return response.body as Item[]
}

/**
 * The clinical inbox (#110): verified results (critical first), unsigned notes and patients'
 * requests. Each opens its authoritative resource; the node leaves out what the policy decision
 * point would not let the user read.
 */
export function InboxPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const items = useQuery({ queryKey: ['inbox'], queryFn: () => inbox(platform.transport), refetchInterval: 30_000 })
  const names = useQueries({
    queries: (items.data ?? []).map((i) => ({
      queryKey: ['fhir', 'Patient', i.patient],
      queryFn: () => platform.fhir.read('Patient', i.patient.replace(/^Patient\//, '')),
      staleTime: 300_000,
    })),
  })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.ibTitle)}</h1>
      {items.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.ibFailed)}
        </p>
      ) : null}
      {items.data?.length === 0 ? <p role="status">{intl.formatMessage(m.ibEmpty)}</p> : null}
      <ol aria-label={intl.formatMessage(m.ibTitle)} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        {(items.data ?? []).map((item, index) => {
          const [type, id] = item.reference.split('/')
          return (
            <li key={item.reference} className={css({ borderBottom: '1px solid', borderColor: 'border', pb: '2' })}>
              <p>
                {intl.formatMessage(m.ibKind, { kind: item.kind })}
                {item.priority === 'stat' ? <strong className={css({ color: 'danger.text', ml: '2' })}>{intl.formatMessage(m.ibUrgent)}</strong> : null}
              </p>
              <p className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.ibFor, { title: item.title, name: patientName(names[index]?.data) || item.patient })}</p>
              <Link to="/records/$type/$id" params={{ type: type ?? '', id: id ?? '' }}>
                {intl.formatMessage(m.ibOpen, { title: item.title })}
              </Link>
            </li>
          )
        })}
      </ol>
    </section>
  )
}
