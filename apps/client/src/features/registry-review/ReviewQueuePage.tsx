import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform, worklistKey } from '../../platform'
import {
  DOCUMENTS,
  REVIEWS,
  ReviewError,
  compare,
  decide,
  decideRegistration,
  reviewQueue,
  submitted,
  type Decision,
  type RegistrationDecision,
  type ReviewItem,
} from './api'

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
const documentLabels = {
  'national-id': messages.rrDocNationalId,
  passport: messages.rrDocPassport,
  'driving-licence': messages.rrDocDrivingLicence,
  'refugee-id': messages.rrDocRefugeeId,
} as const

const kindLabels = {
  'potential-match': messages.rrKindMatch,
  survivorship: messages.rrKindSurvivorship,
  'registration-request': messages.rrKindRegistration,
} as const

const fullName = (patient: { name?: { given?: string[]; family?: string }[] } | null | undefined) =>
  [patient?.name?.[0]?.given?.join(' '), patient?.name?.[0]?.family].filter(Boolean).join(' ')

const table = css({ borderCollapse: 'collapse', '& th, & td': { textAlign: 'start', px: '3', py: '2', borderBottom: '1px solid', borderColor: 'border' } })

/**
 * The registry review queue (patient-identity.md § Review queue; #203): potential matches and
 * survivorship disagreements, the oldest first, overdue after 7 days, and portal registrations
 * that resemble existing patients (#111), linked only after an in-person ID check. Every decision is checked,
 * carried out and audited by the server; the screen enforces nothing on its own.
 */
export function ReviewQueuePage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const queue = useQuery(reviewQueue(platform.transport))
  const [view, setView] = useState<DataTableView>(emptyView)
  const [selected, setSelected] = useState<ReviewItem | undefined>()
  const [document, setDocument] = useState('')
  const decision = useMutation({
    mutationFn: ({ id, choice }: { id: string; choice: Decision | RegistrationDecision }) =>
      typeof choice === 'string' ? decide(platform.transport, id, choice) : decideRegistration(platform.transport, id, choice),
    onSuccess: () => setSelected(undefined),
    onSettled: () => queryClient.invalidateQueries({ queryKey: worklistKey(REVIEWS) }),
  })
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  const columns: DataTableColumn<ReviewItem>[] = [
    {
      id: 'kind',
      header: intl.formatMessage(messages.rrColKind),
      cell: (item) => intl.formatMessage(kindLabels[item.kind]),
      sortValue: (item) => item.kind,
    },
    {
      id: 'queued',
      header: intl.formatMessage(messages.rrColQueued),
      cell: (item) => (item.overdue ? `${time(item.created_at)} · ${intl.formatMessage(messages.rrOverdue)}` : time(item.created_at)),
      sortValue: (item) => item.created_at,
    },
    {
      id: 'summary',
      header: intl.formatMessage(messages.rrColSummary),
      cell: (item) =>
        item.kind === 'potential-match'
          ? fullName(item.source)
          : item.kind === 'registration-request'
            ? fullName(submitted(item.response))
            : `${item.field}: ${item.current.value} / ${item.incoming.value}`,
    },
  ]
  const act = (decided: Decision | RegistrationDecision) => () => {
    if (selected !== undefined) decision.mutate({ id: selected.id, choice: decided })
  }
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '5xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(messages.rrTitle)}</h1>
      {queue.data === undefined ? (
        queue.isPending ? (
          <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
        ) : (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(queue.error instanceof ReviewError && queue.error.status === 403 ? messages.rrForbidden : messages.rrFailed)}
          </p>
        )
      ) : queue.data.length === 0 ? (
        <p role="status">{intl.formatMessage(messages.rrEmpty)}</p>
      ) : (
        <>
          <p className={css({ color: 'fg.muted' })}>{intl.formatMessage(messages.rrHint)}</p>
          <DataTable
            caption={intl.formatMessage(messages.rrTitle)}
            columns={columns}
            data={queue.data}
            getRowId={(item) => item.id}
            view={view}
            onViewChange={setView}
            onRowActivate={(item) => {
              setSelected(item)
              setDocument('')
              decision.reset()
            }}
            height="20rem"
          />
        </>
      )}

      {selected === undefined ? null : (
        <section aria-labelledby="review-item" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <h2 id="review-item" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
            {intl.formatMessage(kindLabels[selected.kind])}
          </h2>
          {selected.kind === 'registration-request' ? (
            <>
              <p>{intl.formatMessage(messages.rrRegistrationHint)}</p>
              <label className={css({ display: 'flex', flexDirection: 'column', gap: '1', maxWidth: 'sm' })}>
                {intl.formatMessage(messages.rrDocument)}
                <select
                  value={document}
                  onChange={(event) => setDocument(event.target.value)}
                  className={css({ minHeight: 'control', px: '2', borderRadius: 'md', border: '1px solid', borderColor: 'border' })}
                >
                  <option value="">{intl.formatMessage(messages.rrChooseDocument)}</option>
                  {DOCUMENTS.map((code) => (
                    <option key={code} value={code}>
                      {intl.formatMessage(documentLabels[code])}
                    </option>
                  ))}
                </select>
              </label>
              {selected.candidates.map((candidate, index) =>
                candidate?.id === undefined ? null : (
                  <section key={candidate.id} aria-label={fullName(candidate) || String(index + 1)} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
                    <table className={table}>
                      <thead>
                        <tr>
                          <th scope="col">{intl.formatMessage(messages.rrField)}</th>
                          <th scope="col">{intl.formatMessage(messages.rrSubmitted)}</th>
                          <th scope="col">{intl.formatMessage(messages.rrExisting)}</th>
                          <th scope="col">{intl.formatMessage(messages.rrAgrees)}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {compare(submitted(selected.response), candidate, platform.registries.ninSystem).map((row) => (
                          <tr key={row.field}>
                            <th scope="row">{row.field}</th>
                            <td>{row.source}</td>
                            <td>{row.candidate}</td>
                            <td>{intl.formatMessage(row.agrees ? messages.rrAgrees : messages.rrDiffers)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    <div>
                      <button
                        type="button"
                        className={button}
                        disabled={decision.isPending || document === ''}
                        onClick={act({ decision: 'link-patient', patient: candidate.id, document })}
                      >
                        {intl.formatMessage(messages.rrLinkPatient)}
                      </button>
                    </div>
                  </section>
                ),
              )}
              <div>
                <button type="button" className={button} disabled={decision.isPending} onClick={act({ decision: 'create-patient' })}>
                  {intl.formatMessage(messages.rrCreatePatient)}
                </button>
              </div>
            </>
          ) : selected.kind === 'potential-match' ? (
            <>
              <p>
                {intl.formatMessage(messages.rrScore)} {selected.score.toFixed(1)}
              </p>
              <table className={table}>
                <thead>
                  <tr>
                    <th scope="col">{intl.formatMessage(messages.rrField)}</th>
                    <th scope="col">{intl.formatMessage(messages.rrNewRecord)}</th>
                    <th scope="col">{intl.formatMessage(messages.rrExisting)}</th>
                    <th scope="col">{intl.formatMessage(messages.rrAgrees)}</th>
                  </tr>
                </thead>
                <tbody>
                  {compare(selected.source, selected.candidate, platform.registries.ninSystem).map((row) => (
                    <tr key={row.field}>
                      <th scope="row">{row.field}</th>
                      <td>{row.source}</td>
                      <td>{row.candidate}</td>
                      <td>{intl.formatMessage(row.agrees ? messages.rrAgrees : messages.rrDiffers)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className={css({ display: 'flex', gap: '3' })}>
                <button type="button" className={button} disabled={decision.isPending} onClick={act('same-person')}>
                  {intl.formatMessage(messages.rrSamePerson)}
                </button>
                <button type="button" className={button} disabled={decision.isPending} onClick={act('different-people')}>
                  {intl.formatMessage(messages.rrDifferentPeople)}
                </button>
              </div>
            </>
          ) : (
            <>
              <table className={table}>
                <thead>
                  <tr>
                    <th scope="col">{selected.field}</th>
                    <th scope="col">{intl.formatMessage(messages.rrTrust)}</th>
                    <th scope="col">{intl.formatMessage(messages.rrSource)}</th>
                  </tr>
                </thead>
                <tbody>
                  {(
                    [
                      [messages.rrCurrent, selected.current],
                      [messages.rrIncoming, selected.incoming],
                    ] as const
                  ).map(([label, claim]) => (
                    <tr key={label.id}>
                      <th scope="row">
                        {intl.formatMessage(label)}: {claim.value}
                      </th>
                      <td>{claim.trust}</td>
                      <td>{claim.source}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className={css({ display: 'flex', gap: '3' })}>
                <button type="button" className={button} disabled={decision.isPending} onClick={act('keep-current')}>
                  {intl.formatMessage(messages.rrKeepCurrent)}
                </button>
                <button type="button" className={button} disabled={decision.isPending} onClick={act('take-incoming')}>
                  {intl.formatMessage(messages.rrTakeIncoming)}
                </button>
              </div>
            </>
          )}
          {decision.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {intl.formatMessage(messages.rrDecideFailed)}
            </p>
          ) : null}
        </section>
      )}
    </section>
  )
}
