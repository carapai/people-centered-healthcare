import type { ResourceType } from '@health/fhir-types'
import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { inboxMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { isPendingProposal } from '../requests/api'
import { RequestActions } from '../requests/RequestActions'

/** What a person reads of a coded element, if anything. */
function shown(value: unknown): string | undefined {
  if (typeof value === 'string') return value
  if (typeof value !== 'object' || value === null) return undefined
  const concept = value as { text?: string; coding?: { display?: string; code?: string }[]; concept?: unknown }
  return concept.text ?? concept.coding?.[0]?.display ?? concept.coding?.[0]?.code ?? shown(concept.concept)
}

/**
 * One authoritative resource (#110): what an inbox item links to, read through the FHIR API, so
 * the policy decision point decides it as any read.
 */
export function RecordPage({ type, id }: { type: string; id: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const record = useQuery({ queryKey: ['fhir', type, id], queryFn: () => platform.fhir.read(type as ResourceType, id) })
  const resource = record.data as Record<string, unknown> | undefined
  const [outcome, setOutcome] = useState('')
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '3', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{type}</h1>
      {record.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.ibNotFound)}
        </p>
      ) : null}
      {resource === undefined ? null : (
        <>
          <dl className={css({ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '1', columnGap: '3' })}>
            {(
              [
                [intl.formatMessage(m.ibWhat), shown(resource['code']) ?? shown(resource['title']) ?? shown(resource['medication']) ?? shown(resource['description'])],
                [intl.formatMessage(m.ibStatus), shown(resource['status'])],
                [intl.formatMessage(m.ibWhen), shown(resource['issued']) ?? shown(resource['date']) ?? shown(resource['authoredOn']) ?? shown(resource['start']) ?? shown(resource['created'])],
              ] as const
            )
              .filter(([, value]) => value !== undefined)
              .map(([label, value]) => (
                <div key={label} className={css({ display: 'contents' })}>
                  <dt className={css({ fontWeight: 'semibold' })}>{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
          </dl>
          {outcome === '' ? null : <p role="status">{outcome}</p>}
          {isPendingProposal(resource) ? <RequestActions key={`${type}/${id}`} reference={`${type}/${id}`} appointment={type === 'Appointment'} onDone={setOutcome} /> : null}
          <details>
            <summary>{intl.formatMessage(m.ibRaw)}</summary>
            <pre className={css({ whiteSpace: 'pre-wrap', fontSize: 'sm' })}>{JSON.stringify(resource, null, 2)}</pre>
          </details>
        </>
      )}
    </section>
  )
}
