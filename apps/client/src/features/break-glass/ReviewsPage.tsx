import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform, worklistKey } from '../../platform'
import { REVIEWS, decide, reviewsQuery, type Grant } from './api'
import { GrantTable, formatTime } from './GrantTable'
import { ListState, action, heading, page } from './states'

/**
 * The privacy officer's break-glass review queue (offline-auth.md § Break-glass): every grant,
 * the earliest due first, with overdue ones flagged. Selecting a row opens its decision; an
 * access that was not appropriate needs a note.
 */
export function ReviewsPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const reviews = useQuery(reviewsQuery(transport))
  const [selected, setSelected] = useState<Grant | undefined>()
  const [note, setNote] = useState('')
  const decision = useMutation({
    mutationFn: ({ id, appropriate }: { id: string; appropriate: boolean }) => decide(transport, id, appropriate, note.trim()),
    onSuccess: () => {
      setSelected(undefined)
      setNote('')
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: worklistKey(REVIEWS) }),
  })
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(messages.bgReviewsTitle)}</h1>
      {reviews.data === undefined ? (
        <ListState loading={reviews.isPending} error={reviews.error} />
      ) : reviews.data.length === 0 ? (
        <p role="status">{intl.formatMessage(messages.bgReviewsEmpty)}</p>
      ) : (
        <>
          <p className={css({ color: 'fg.muted' })}>{intl.formatMessage(messages.bgSelectHint)}</p>
          <GrantTable
            caption={intl.formatMessage(messages.bgReviewsTitle)}
            grants={reviews.data}
            onRowActivate={(g) => {
              setSelected(g)
              decision.reset()
            }}
            extra={[
              { id: 'facility', header: intl.formatMessage(messages.bgColFacility), cell: (g) => g.facility, sortValue: (g) => g.facility },
              {
                id: 'due',
                header: intl.formatMessage(messages.bgColDue),
                cell: (g) => (g.overdue === true ? `${formatTime(intl, g.review_due)} · ${intl.formatMessage(messages.bgOverdue)}` : formatTime(intl, g.review_due)),
                sortValue: (g) => g.review_due,
              },
            ]}
          />
        </>
      )}
      {selected === undefined ? null : (
        <form
          className={css({ display: 'flex', flexDirection: 'column', gap: '3', maxWidth: 'lg' })}
          onSubmit={(event) => event.preventDefault()}
          aria-labelledby="review-heading"
        >
          <h2 id="review-heading" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
            {intl.formatMessage(messages.bgDecideHeading)}
          </h2>
          <dl className={css({ display: 'grid', gridTemplateColumns: 'max-content 1fr', columnGap: '4', rowGap: '1' })}>
            <dt>{intl.formatMessage(messages.bgColPatient)}</dt>
            <dd>{selected.patient}</dd>
            <dt>{intl.formatMessage(messages.bgColUser)}</dt>
            <dd>{selected.account}</dd>
            <dt>{intl.formatMessage(messages.bgColReason)}</dt>
            <dd>{selected.justification}</dd>
          </dl>
          <Field.Root>
            <Field.Label>{intl.formatMessage(messages.bgNote)}</Field.Label>
            <Field.Textarea value={note} onChange={(event) => setNote(event.currentTarget.value)} />
            <Field.HelperText>{intl.formatMessage(messages.bgNoteHelp)}</Field.HelperText>
          </Field.Root>
          {decision.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {intl.formatMessage(messages.bgDecideFailed)}
            </p>
          ) : null}
          <div className={css({ display: 'flex', gap: '3' })}>
            <button type="button" className={action} disabled={decision.isPending} onClick={() => decision.mutate({ id: selected.id, appropriate: true })}>
              {intl.formatMessage(messages.bgAppropriate)}
            </button>
            <button
              type="button"
              className={action}
              disabled={decision.isPending || note.trim() === ''}
              onClick={() => decision.mutate({ id: selected.id, appropriate: false })}
            >
              {intl.formatMessage(messages.bgInappropriate)}
            </button>
          </div>
        </form>
      )}
    </section>
  )
}
