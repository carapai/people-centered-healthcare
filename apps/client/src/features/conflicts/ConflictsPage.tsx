import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { conflictMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button } from '../reception/styles'
import { ConflictError, candidateFor, conflictQuery, conflictsKey, conflictsQuery, content, resolve, type ConflictSummary, type Decision } from './api'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '6xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const branches = css({ display: 'grid', gridTemplateColumns: { base: '1fr', md: '1fr 1fr' }, gap: '3' })
const pre = css({ fontFamily: 'mono', fontSize: 'sm', whiteSpace: 'pre-wrap', bg: 'bg.subtle', p: '2', borderRadius: 'md', overflowX: 'auto' })
const editor = css({ fontFamily: 'mono', fontSize: 'sm', minHeight: '48', p: '2', borderRadius: 'md', border: '1px solid', borderColor: 'border' })
const actions = css({ display: 'flex', flexWrap: 'wrap', gap: '2' })

const pretty = (value: unknown) => JSON.stringify(value, null, 2)

/**
 * The Conflict Record review queue (#82; synchronization.md § Conflict Record and review queue):
 * the open records, oldest first and overdue after 7 days; for one record, the current version
 * here beside the version the facility sent, and the decision: keep the current version, take
 * the facility's, or write a corrected one on top. The server keeps both branches, checks and
 * audits the decision, and syncs it back; nothing is merged here.
 */
export function ConflictsPage() {
  const [open, setOpen] = useState<ConflictSummary | undefined>()
  const [resolved, setResolved] = useState(false)
  return open === undefined ? (
    <Queue
      resolved={resolved}
      onOpen={(record) => {
        setResolved(false)
        setOpen(record)
      }}
    />
  ) : (
    <Record
      summary={open}
      onClose={(done) => {
        setResolved(done)
        setOpen(undefined)
      }}
    />
  )
}

function Queue({ resolved, onOpen }: { resolved: boolean; onOpen: (record: ConflictSummary) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queue = useQuery(conflictsQuery(platform.transport))
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.cfTitle)}</h1>
      {resolved ? <p role="status">{intl.formatMessage(m.cfResolved)}</p> : null}
      {queue.isPending ? <p role="status">{intl.formatMessage(m.cfLoading)}</p> : null}
      {queue.error instanceof ConflictError && queue.error.status === 403 ? <p role="alert">{intl.formatMessage(m.cfRefused)}</p> : null}
      {queue.data?.length === 0 ? <p>{intl.formatMessage(m.cfNone)}</p> : null}
      <ol aria-label={intl.formatMessage(m.cfTitle)} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {(queue.data ?? []).map((record) => (
          <li key={record.id} className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
            <button type="button" className={button} onClick={() => onOpen(record)}>
              {intl.formatMessage(m.cfOpen, { resource: record.items[0]?.resource ?? record.id, peer: record.peer, opened: time(record.openedAt) })}
            </button>
            {record.overdue ? <strong className={css({ color: 'danger.text' })}>{intl.formatMessage(m.cfOverdue)}</strong> : null}
          </li>
        ))}
      </ol>
    </section>
  )
}

function Record({ summary, onClose }: { summary: ConflictSummary; onClose: (resolved: boolean) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const editorId = useId()
  const detail = useQuery(conflictQuery(platform.transport, summary.id))
  const resource = summary.items[0]?.resource ?? ''
  const head = content(detail.data?.heads[resource])
  const candidate = detail.data === undefined ? undefined : candidateFor(detail.data, resource)
  const [corrected, setCorrected] = useState<string | undefined>()
  const [invalid, setInvalid] = useState(false)
  const decide = useMutation({
    mutationFn: (decision: Decision) => resolve(platform.transport, summary.id, decision),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: conflictsKey })
      onClose(true)
    },
  })
  const writeCorrected = () => {
    try {
      const json = JSON.parse(corrected ?? pretty(head ?? {})) as Record<string, unknown>
      setInvalid(false)
      decide.mutate({ decision: 'write', resource, json })
    } catch {
      setInvalid(true)
    }
  }
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.cfRecord, { resource })}</h1>
      <p>{intl.formatMessage(m.cfPolicy, { policy: summary.policy })}</p>
      <div>
        <button type="button" className={button} onClick={() => onClose(false)}>
          {intl.formatMessage(m.cfBack)}
        </button>
      </div>
      {detail.isPending ? <p role="status">{intl.formatMessage(m.cfLoading)}</p> : null}
      <div className={branches}>
        <section aria-label={intl.formatMessage(m.cfHead)}>
          <h2 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.cfHead)}</h2>
          <pre className={pre}>{pretty(head ?? null)}</pre>
        </section>
        <section aria-label={intl.formatMessage(m.cfCandidate, { peer: summary.peer })}>
          <h2 className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.cfCandidate, { peer: summary.peer })}</h2>
          {candidate === undefined ? <p>{intl.formatMessage(m.cfNoCandidate)}</p> : <pre className={pre}>{pretty(content(candidate.body))}</pre>}
        </section>
      </div>
      {decide.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.cfFailed, { reason: decide.error instanceof ConflictError ? decide.error.reason : '' })}
        </p>
      ) : null}
      <div className={actions}>
        <button type="button" className={button} disabled={decide.isPending || detail.data === undefined} onClick={() => decide.mutate({ decision: 'keep-head', resource })}>
          {intl.formatMessage(m.cfKeepHead)}
        </button>
        <button
          type="button"
          className={button}
          disabled={decide.isPending || candidate === undefined}
          onClick={() => candidate !== undefined && decide.mutate({ decision: 'write', resource, json: content(candidate.body) ?? {} })}
        >
          {intl.formatMessage(m.cfTakeCandidate)}
        </button>
      </div>
      <label htmlFor={editorId} className={css({ fontWeight: 'semibold' })}>
        {intl.formatMessage(m.cfCorrected)}
      </label>
      <textarea id={editorId} className={editor} value={corrected ?? pretty(head ?? {})} onChange={(e) => setCorrected(e.target.value)} />
      {invalid ? <p role="alert">{intl.formatMessage(m.cfNotJson)}</p> : null}
      <div>
        <button type="button" className={button} disabled={decide.isPending || detail.data === undefined} onClick={writeCorrected}>
          {intl.formatMessage(m.cfWriteCorrected)}
        </button>
      </div>
    </section>
  )
}
