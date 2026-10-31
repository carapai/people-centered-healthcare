import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform, worklistKey } from '../../platform'
import { REVIEWS, ReviewError, compare, decide, reviewQueue, type Decision, type ReviewItem } from './api'

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
const table = css({ borderCollapse: 'collapse', '& th, & td': { textAlign: 'start', px: '3', py: '2', borderBottom: '1px solid', borderColor: 'border' } })

/**
 * The registry review queue (patient-identity.md § Review queue; #203): potential matches and
 * survivorship disagreements, the oldest first, overdue after 7 days. Every decision is checked,
 * carried out and audited by the server; the screen enforces nothing on its own.
 */
export function ReviewQueuePage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const queue = useQuery(reviewQueue(platform.transport))
  const [view, setView] = useState<DataTableView>(emptyView)
  const [selected, setSelected] = useState<ReviewItem | undefined>()
  const decision = useMutation({
    mutationFn: ({ id, choice }: { id: string; choice: Decision }) => decide(platform.transport, id, choice),
    onSuccess: () => setSelected(undefined),
    onSettled: () => queryClient.invalidateQueries({ queryKey: worklistKey(REVIEWS) }),
  })
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  const columns: DataTableColumn<ReviewItem>[] = [
    {
      id: 'kind',
      header: intl.formatMessage(messages.rrColKind),
      cell: (item) => intl.formatMessage(item.kind === 'potential-match' ? messages.rrKindMatch : messages.rrKindSurvivorship),
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
          ? [item.source?.name?.[0]?.given?.join(' '), item.source?.name?.[0]?.family].filter(Boolean).join(' ')
          : `${item.field}: ${item.current.value} / ${item.incoming.value}`,
    },
  ]
  const act = (decided: Decision) => () => {
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
              decision.reset()
            }}
            height="20rem"
          />
        </>
      )}

      {selected === undefined ? null : (
        <section aria-labelledby="review-item" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <h2 id="review-item" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
            {intl.formatMessage(selected.kind === 'potential-match' ? messages.rrKindMatch : messages.rrKindSurvivorship)}
          </h2>
          {selected.kind === 'potential-match' ? (
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
