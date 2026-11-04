import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { worklistMessages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { WorkflowError, claim, complete, definitionsQuery, release, startInstance, worklistQuery, worklistQueryKey, worklists, type WorkTask } from './api'

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
const field = css({ display: 'flex', flexDirection: 'column', gap: '1', maxWidth: 'sm' })
const control = css({ minHeight: 'control', px: '2', borderRadius: 'md', border: '1px solid', borderColor: 'border' })

const priorityRank = { routine: 0, urgent: 1, asap: 2, stat: 3 } as const

/**
 * Worklists (workflow-engine.md § Worklists and queues; #99): a worklist's open tasks, most
 * urgent first, then by arrival. Taking a task claims it (it lapses after 15 idle minutes),
 * and completing it sends one of the events its step takes. Every command is a workflow command
 * the server checks and records; the screen enforces nothing on its own.
 */
export function WorklistsPage() {
  const intl = useIntl()
  const ids = { definition: useId(), context: useId(), list: useId() }
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const definitions = useQuery(definitionsQuery(platform.transport))
  // Who is signed in, to tell their own claims from a colleague's.
  const session = useQuery({
    queryKey: ['session'],
    queryFn: ({ signal }) => platform.transport.request({ method: 'GET', path: '/session', signal }),
  })
  const me = session.data?.status === 200 ? (session.data.body as { account?: string }).account : undefined
  const [list, setList] = useState('')
  const [definition, setDefinition] = useState('')
  const [context, setContext] = useState('')
  const [view, setView] = useState<DataTableView>(emptyView)
  const [selected, setSelected] = useState<WorkTask | undefined>()
  const tasks = useQuery({ ...worklistQuery(platform.transport, list), enabled: list !== '' })
  const refresh = () => queryClient.invalidateQueries({ queryKey: worklistQueryKey(list) })

  const start = useMutation({
    mutationFn: () => startInstance(platform.transport, definition, context.trim()),
    onSuccess: () => setContext(''),
    onSettled: refresh,
  })
  const act = useMutation({
    mutationFn: ({ task, run }: { task: WorkTask; run: 'claim' | 'release' | { event: string } }) =>
      run === 'claim' ? claim(platform.transport, task) : run === 'release' ? release(platform.transport, task) : complete(platform.transport, task, run.event),
    onSuccess: (_, { run }) => {
      if (typeof run === 'object') setSelected(undefined)
    },
    onSettled: async () => {
      await refresh()
    },
  })

  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  const status = (task: WorkTask) =>
    intl.formatMessage(task.owner === null ? worklistMessages.wlReady : task.owner === me ? worklistMessages.wlMine : worklistMessages.wlTaken)
  const columns: DataTableColumn<WorkTask>[] = [
    { id: 'step', header: intl.formatMessage(worklistMessages.wlColStep), cell: (t) => t.state, sortValue: (t) => t.state },
    { id: 'about', header: intl.formatMessage(worklistMessages.wlColAbout), cell: (t) => t.context },
    { id: 'priority', header: intl.formatMessage(worklistMessages.wlColPriority), cell: (t) => t.priority, sortValue: (t) => priorityRank[t.priority] },
    { id: 'arrived', header: intl.formatMessage(worklistMessages.wlColArrived), cell: (t) => time(t.authoredOn), sortValue: (t) => t.authoredOn },
    { id: 'status', header: intl.formatMessage(worklistMessages.wlColStatus), cell: status },
  ]
  // The selected task as the worklist now shows it, after a claim or a release.
  const current = selected === undefined ? undefined : (tasks.data?.find((t) => t.id === selected.id) ?? selected)
  const refused = (error: unknown) => intl.formatMessage(worklistMessages.wlActionFailed, { reason: error instanceof WorkflowError ? error.reason || String(error.status) : '' })

  if (definitions.data === undefined) {
    return (
      <section className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
        <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(worklistMessages.wlTitle)}</h1>
        {definitions.isPending ? (
          <p role="status">{intl.formatMessage(worklistMessages.wlLoading)}</p>
        ) : (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(definitions.error instanceof WorkflowError && definitions.error.status === 403 ? worklistMessages.wlForbidden : worklistMessages.wlFailed)}
          </p>
        )}
      </section>
    )
  }
  const lists = worklists(definitions.data)
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '5xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(worklistMessages.wlTitle)}</h1>
      {definitions.data.length === 0 ? <p role="status">{intl.formatMessage(worklistMessages.wlNone)}</p> : null}

      <section aria-labelledby="wl-start" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        <h2 id="wl-start" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
          {intl.formatMessage(worklistMessages.wlStartTitle)}
        </h2>
        <form
          className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}
          onSubmit={(event) => {
            event.preventDefault()
            start.mutate()
          }}
        >
          <div className={field}>
            <label htmlFor={ids.definition}>{intl.formatMessage(worklistMessages.wlDefinition)}</label>
            <select id={ids.definition} value={definition} onChange={(e) => setDefinition(e.target.value)} className={control}>
              <option value="">{intl.formatMessage(worklistMessages.wlDefinition)}</option>
              {definitions.data.map((d) => (
                <option key={d.definition} value={d.definition}>
                  {d.definition}
                </option>
              ))}
            </select>
          </div>
          <div className={field}>
            <label htmlFor={ids.context}>{intl.formatMessage(worklistMessages.wlContext)}</label>
            <input id={ids.context} value={context} onChange={(e) => setContext(e.target.value)} className={control} />
          </div>
          <div>
            <button type="submit" className={button} disabled={start.isPending || definition === '' || !context.includes('/')}>
              {intl.formatMessage(worklistMessages.wlStart)}
            </button>
          </div>
          {start.isSuccess ? <p role="status">{intl.formatMessage(worklistMessages.wlStarted)}</p> : null}
          {start.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {intl.formatMessage(worklistMessages.wlStartFailed)}
            </p>
          ) : null}
        </form>
      </section>

      <div className={field}>
        <label htmlFor={ids.list}>{intl.formatMessage(worklistMessages.wlWorklist)}</label>
        <select
          id={ids.list}
          value={list}
          onChange={(e) => {
            setList(e.target.value)
            setSelected(undefined)
          }}
          className={control}
        >
          <option value="">{intl.formatMessage(worklistMessages.wlChoose)}</option>
          {lists.map((key) => (
            <option key={key} value={key}>
              {key}
            </option>
          ))}
        </select>
      </div>

      {list === '' ? null : tasks.data === undefined ? (
        tasks.isPending ? (
          <p role="status">{intl.formatMessage(worklistMessages.wlLoading)}</p>
        ) : (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(worklistMessages.wlFailed)}
          </p>
        )
      ) : tasks.data.length === 0 ? (
        <p role="status">{intl.formatMessage(worklistMessages.wlEmpty)}</p>
      ) : (
        <>
          <p className={css({ color: 'fg.muted' })}>{intl.formatMessage(worklistMessages.wlHint)}</p>
          <DataTable
            caption={list}
            columns={columns}
            data={tasks.data}
            getRowId={(t) => t.id}
            view={view}
            onViewChange={setView}
            onRowActivate={(t) => {
              setSelected(t)
              act.reset()
            }}
            height="20rem"
          />
        </>
      )}

      {current === undefined ? null : (
        <section aria-labelledby="wl-task" className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <h2 id="wl-task" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
            {current.state} · {current.context}
          </h2>
          <p>{status(current)}</p>
          <div className={css({ display: 'flex', gap: '2', flexWrap: 'wrap' })}>
            {current.owner === null ? (
              <button type="button" className={button} disabled={act.isPending} onClick={() => act.mutate({ task: current, run: 'claim' })}>
                {intl.formatMessage(worklistMessages.wlClaim)}
              </button>
            ) : (
              <button type="button" className={button} disabled={act.isPending} onClick={() => act.mutate({ task: current, run: 'release' })}>
                {intl.formatMessage(worklistMessages.wlRelease)}
              </button>
            )}
            {current.events.map((event) => (
              <button key={event} type="button" className={button} disabled={act.isPending} onClick={() => act.mutate({ task: current, run: { event } })}>
                {intl.formatMessage(worklistMessages.wlComplete, { event })}
              </button>
            ))}
          </div>
          {act.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {refused(act.error)}
            </p>
          ) : null}
        </section>
      )}
    </section>
  )
}
